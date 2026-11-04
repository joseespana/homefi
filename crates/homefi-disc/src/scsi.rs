//! MMC (SCSI multimedia) commands shared by the Linux (`SG_IO`) and Windows (SPTI)
//! transports: CDB builders, response parsers and a [`SectorSource`] over any
//! [`ScsiTransport`]. Pure Rust and drive-free, so every byte is unit-tested on any OS.
//! (macOS has its own ioctls and does not use this.)
#![cfg_attr(not(any(target_os = "linux", target_os = "windows")), allow(dead_code))]

use std::io::ErrorKind;

use crate::cdtext::{parse_cd_text, CdText};
use crate::drives::Device;
use crate::error::{DiscError, Result};
use crate::source::{Capabilities, ReadFlags, ReadSpeed, SectorSource, C2_BYTES, CHUNK_SECTORS, SECTOR_BYTES};
use crate::toc::{RawTocEntry, Toc};

const OP_READ_TOC: u8 = 0x43;
const OP_READ_CD: u8 = 0xBE;
const OP_READ_SUB_CHANNEL: u8 = 0x42;
const OP_SET_CD_SPEED: u8 = 0xBB;
const OP_READ_DISC_STRUCTURE: u8 = 0xAD;

const TOC_FORMAT_TOC: u8 = 0;
const TOC_FORMAT_CD_TEXT: u8 = 5;
const SUBCHANNEL_MCN: u8 = 0x02;
const SUBCHANNEL_ISRC: u8 = 0x03;
const DVD_STRUCTURE_COPYRIGHT: u8 = 0x01;

/// READ CD: expected sector type CD-DA.
const READ_CD_TYPE_CDDA: u8 = 0x01 << 2;
/// READ CD main-channel selection: user data (the audio).
const READ_CD_USER_DATA: u8 = 0x10;
/// READ CD error info: 294 bytes of C2 error bits after the audio.
const READ_CD_C2_BITS: u8 = 0x01 << 1;
/// Largest READ TOC answer: 4-byte header and 100 eight-byte entries.
const TOC_BYTES: usize = 4 + 100 * 8;
const LEADOUT_TRACK: u8 = 0xAA;
const CD_TEXT_HEADER: usize = 4;
/// Largest CD-Text answer worth asking for: 32 KiB covers every real disc.
const CD_TEXT_MAX: usize = 32 * 1024;
const SUBCHANNEL_BYTES: usize = 24;

pub(crate) enum ScsiError {
    /// The drive answered CHECK CONDITION.
    Check {
        key: u8,
        asc: u8,
        ascq: u8,
    },
    Os(std::io::Error),
}

/// Sends one command to the drive and fills `data` (data-in) or sends nothing (`data` empty).
/// Returns the bytes transferred when the OS reports it, else `data.len()`.
pub(crate) trait ScsiTransport: Send {
    fn execute(&mut self, cdb: &[u8], data: &mut [u8]) -> std::result::Result<usize, ScsiError>;
}

/// CHECK CONDITION with the key / ASC / ASCQ out of fixed (0x70, 0x71) or descriptor (0x72, 0x73)
/// sense data.
pub(crate) fn check_condition(sense: &[u8]) -> ScsiError {
    match sense.first().map(|b| b & 0x7F) {
        Some(0x70 | 0x71) if sense.len() >= 14 => {
            ScsiError::Check { key: sense[2] & 0x0F, asc: sense[12], ascq: sense[13] }
        }
        Some(0x72 | 0x73) if sense.len() >= 4 => {
            ScsiError::Check { key: sense[1] & 0x0F, asc: sense[2], ascq: sense[3] }
        }
        _ => ScsiError::Check { key: 0, asc: 0, ascq: 0 },
    }
}

pub(crate) fn map_error(error: ScsiError, lba: u32) -> DiscError {
    match error {
        // NOT READY / MEDIUM NOT PRESENT
        ScsiError::Check { key: 0x02, asc: 0x3A, .. } => DiscError::NoDisc,
        // UNIT ATTENTION / NOT READY TO READY CHANGE (disc swapped)
        ScsiError::Check { key: 0x06, asc: 0x28, .. } => DiscError::Removed,
        ScsiError::Check { key, asc, ascq } => {
            DiscError::Read { lba, message: format!("SCSI sense {key:02X}/{asc:02X}/{ascq:02X}") }
        }
        ScsiError::Os(e) if e.kind() == ErrorKind::PermissionDenied => DiscError::Permission { hint: String::new() },
        ScsiError::Os(e) => DiscError::Read { lba, message: e.to_string() },
    }
}

// ---- CDB builders -------------------------------------------------------------------------

pub(crate) fn cdb_read_toc(format: u8, alloc: u16) -> [u8; 10] {
    let len = alloc.to_be_bytes();
    // Byte 1 bit 1 (MSF) stays 0: addresses come back as LBA.
    [OP_READ_TOC, 0, format & 0x0F, 0, 0, 0, 0, len[0], len[1], 0]
}

pub(crate) fn cdb_read_cd(lba: u32, count: u16, with_c2: bool) -> [u8; 12] {
    let (a, n) = (lba.to_be_bytes(), u32::from(count).to_be_bytes());
    let flags = READ_CD_USER_DATA | if with_c2 { READ_CD_C2_BITS } else { 0 };
    [OP_READ_CD, READ_CD_TYPE_CDDA, a[0], a[1], a[2], a[3], n[1], n[2], n[3], flags, 0, 0]
}

pub(crate) fn cdb_read_sub_channel(format: u8, track: u8) -> [u8; 10] {
    // SubQ = 1 (byte 2 bit 6), LBA addressing, answer limited to one data block.
    [OP_READ_SUB_CHANNEL, 0, 0x40, format, 0, 0, track, 0, SUBCHANNEL_BYTES as u8, 0]
}

pub(crate) fn cdb_set_cd_speed(read_kbps: u16) -> [u8; 12] {
    let speed = read_kbps.to_be_bytes();
    [OP_SET_CD_SPEED, 0, speed[0], speed[1], 0xFF, 0xFF, 0, 0, 0, 0, 0, 0]
}

pub(crate) fn cdb_read_dvd_copyright() -> [u8; 12] {
    let alloc = 8u16.to_be_bytes();
    [OP_READ_DISC_STRUCTURE, 0, 0, 0, 0, 0, 0, DVD_STRUCTURE_COPYRIGHT, alloc[0], alloc[1], 0, 0]
}

// ---- response parsers ---------------------------------------------------------------------

/// Parses READ TOC format 0 (LBA addressing) into a [`Toc`].
pub(crate) fn parse_toc(data: &[u8]) -> Result<Toc> {
    if data.len() < 4 {
        return Err(DiscError::NoDisc);
    }
    let declared = usize::from(u16::from_be_bytes([data[0], data[1]])) + 2;
    let end = declared.min(data.len());
    let mut entries = Vec::new();
    let mut leadout = None;
    for row in data[4..end].chunks_exact(8) {
        let (control, number) = (row[1] & 0x0F, row[2]);
        let lba = u32::from_be_bytes([row[4], row[5], row[6], row[7]]);
        if number == LEADOUT_TRACK {
            leadout = Some(lba);
        } else if (1..=99).contains(&number) {
            entries.push(RawTocEntry { number, control, start_lba: lba });
        }
    }
    let leadout = leadout.ok_or(DiscError::NoDisc)?;
    if entries.is_empty() {
        return Err(DiscError::NoDisc);
    }
    Ok(Toc::from_entries(&entries, leadout))
}

/// Parses a raw ("full") TOC, READ TOC format 2: 4-byte header and 11-byte descriptors
/// `session, adr/control, tno, point, min, sec, frame, zero, pmin, psec, pframe`. Tracks are
/// points 1..=99, point 0xA2 is a session's lead-out (the last one is the disc's).
#[cfg(any(target_os = "macos", test))]
pub(crate) fn parse_full_toc(data: &[u8]) -> Result<Toc> {
    const DESCRIPTOR: usize = 11;
    if data.len() < 4 {
        return Err(DiscError::NoDisc);
    }
    let declared = usize::from(u16::from_be_bytes([data[0], data[1]])) + 2;
    let end = declared.min(data.len());
    let mut entries: Vec<RawTocEntry> = Vec::new();
    let mut leadout = 0u32;
    for row in data[4..end].chunks_exact(DESCRIPTOR) {
        let (adr, control, point) = (row[1] >> 4, row[1] & 0x0F, row[3]);
        if adr != 1 {
            continue;
        }
        let start = (u32::from(row[8]) * 60 + u32::from(row[9])) * 75 + u32::from(row[10]);
        let start = start.saturating_sub(crate::source::PREGAP_SECTORS);
        match point {
            1..=99 if !entries.iter().any(|e| e.number == point) => {
                entries.push(RawTocEntry { number: point, control, start_lba: start })
            }
            0xA2 => leadout = leadout.max(start),
            _ => {}
        }
    }
    if entries.is_empty() || leadout == 0 {
        return Err(DiscError::NoDisc);
    }
    Ok(Toc::from_entries(&entries, leadout))
}

/// Letters and digits of an ISRC/MCN field (NUL or space padded), or `None` when empty or all
/// zeros.
pub(crate) fn valid_code(raw: &[u8]) -> Option<String> {
    let code: String = raw.iter().map(|b| char::from(*b)).collect();
    let code = code.trim_matches(|c: char| c == '\0' || c == ' ');
    // A NUL inside means the string ended there.
    let code = code.split('\0').next().unwrap_or("");
    let usable = !code.is_empty() && code.chars().all(|c| c.is_ascii_alphanumeric());
    (usable && code.chars().any(|c| c != '0')).then(|| code.to_owned())
}

fn code_from(data: &[u8], valid_bit_at: usize, text: std::ops::Range<usize>) -> Option<String> {
    if data.len() < text.end || data[valid_bit_at] & 0x80 == 0 {
        return None;
    }
    valid_code(&data[text])
}

/// Data block of READ SUB-CHANNEL starts after the 4-byte header: the code is at 9.. and its
/// "valid" bit is byte 8.
pub(crate) fn parse_isrc(data: &[u8]) -> Option<String> {
    code_from(data, 8, 9..21)
}

pub(crate) fn parse_mcn(data: &[u8]) -> Option<String> {
    code_from(data, 8, 9..22)
}

/// Copyright protection system type of READ DISC STRUCTURE format 1 (0 = none).
pub(crate) fn parse_dvd_copyright(data: &[u8]) -> Option<u8> {
    (data.len() >= 6).then(|| data[4])
}

// ---- the source -----------------------------------------------------------------------------

pub(crate) struct ScsiSource<T> {
    transport: T,
    capabilities: Capabilities,
    /// Audio + C2 interleaved as the drive sends it; sized once for the largest window.
    interleaved: Vec<u8>,
}

impl<T: ScsiTransport> ScsiSource<T> {
    pub(crate) fn new(transport: T) -> Self {
        ScsiSource {
            transport,
            capabilities: Capabilities::default(),
            interleaved: vec![0; usize::from(CHUNK_SECTORS) * (SECTOR_BYTES + C2_BYTES)],
        }
    }

    /// Finds out what the drive can do. Costs a few reads, so it runs when a session opens, not
    /// when drives are only being listed.
    pub(crate) fn prepare(&mut self) {
        if let Ok(toc) = self.toc() {
            self.capabilities.c2 = self.probe_c2(&toc);
            self.capabilities.cd_text = true;
        }
    }

    /// C2 is supported when a READ CD asking for it succeeds on a real audio sector.
    fn probe_c2(&mut self, toc: &Toc) -> bool {
        let Some(track) = toc.audio_tracks().next() else {
            return false;
        };
        let mut buffer = vec![0u8; SECTOR_BYTES + C2_BYTES];
        self.transport.execute(&cdb_read_cd(track.start_lba, 1, true), &mut buffer).is_ok()
    }

    fn run(&mut self, cdb: &[u8], data: &mut [u8], lba: u32) -> Result<usize> {
        self.transport.execute(cdb, data).map_err(|e| map_error(e, lba))
    }

    pub(crate) fn dvd_copyright(&mut self) -> Option<u8> {
        let mut data = [0u8; 8];
        self.run(&cdb_read_dvd_copyright(), &mut data, 0).ok()?;
        parse_dvd_copyright(&data)
    }
}

impl<T: ScsiTransport> SectorSource for ScsiSource<T> {
    fn toc(&mut self) -> Result<Toc> {
        let mut data = vec![0u8; TOC_BYTES];
        let got = self.run(&cdb_read_toc(TOC_FORMAT_TOC, TOC_BYTES as u16), &mut data, 0)?;
        parse_toc(&data[..got.min(data.len())])
    }

    fn read_audio(&mut self, lba: u32, count: u16, pcm: &mut [u8], c2: Option<&mut [u8]>) -> Result<ReadFlags> {
        let sectors = usize::from(count);
        assert!(pcm.len() >= sectors * SECTOR_BYTES, "pcm buffer too small");
        match c2 {
            Some(c2) if self.capabilities.c2 && sectors <= usize::from(CHUNK_SECTORS) => {
                let stride = SECTOR_BYTES + C2_BYTES;
                let mut buffer = std::mem::take(&mut self.interleaved);
                let result = self.run(&cdb_read_cd(lba, count, true), &mut buffer[..sectors * stride], lba);
                if result.is_ok() {
                    for i in 0..sectors {
                        let from = i * stride;
                        pcm[i * SECTOR_BYTES..(i + 1) * SECTOR_BYTES]
                            .copy_from_slice(&buffer[from..from + SECTOR_BYTES]);
                        c2[i * C2_BYTES..(i + 1) * C2_BYTES]
                            .copy_from_slice(&buffer[from + SECTOR_BYTES..from + stride]);
                    }
                }
                self.interleaved = buffer;
                result.map(|_| ReadFlags { c2: true })
            }
            _ => {
                self.run(&cdb_read_cd(lba, count, false), &mut pcm[..sectors * SECTOR_BYTES], lba)?;
                Ok(ReadFlags::default())
            }
        }
    }

    fn cd_text(&mut self) -> Result<Option<CdText>> {
        let mut header = [0u8; CD_TEXT_HEADER];
        if self.run(&cdb_read_toc(TOC_FORMAT_CD_TEXT, CD_TEXT_HEADER as u16), &mut header, 0).is_err() {
            // Drives without CD-Text support answer ILLEGAL REQUEST.
            return Ok(None);
        }
        let length = usize::from(u16::from_be_bytes([header[0], header[1]])) + 2;
        if length <= CD_TEXT_HEADER {
            return Ok(None);
        }
        let mut data = vec![0u8; length.min(CD_TEXT_MAX)];
        let Ok(got) = self.run(&cdb_read_toc(TOC_FORMAT_CD_TEXT, data.len() as u16), &mut data, 0) else {
            return Ok(None);
        };
        Ok(parse_cd_text(&data[..got.min(data.len())]))
    }

    fn isrc(&mut self, track: u8) -> Result<Option<String>> {
        let mut data = [0u8; SUBCHANNEL_BYTES];
        self.run(&cdb_read_sub_channel(SUBCHANNEL_ISRC, track), &mut data, 0)?;
        Ok(parse_isrc(&data))
    }

    fn mcn(&mut self) -> Result<Option<String>> {
        let mut data = [0u8; SUBCHANNEL_BYTES];
        self.run(&cdb_read_sub_channel(SUBCHANNEL_MCN, 0), &mut data, 0)?;
        Ok(parse_mcn(&data))
    }

    fn set_speed(&mut self, speed: ReadSpeed) -> Result<()> {
        let Some(kbps) = speed.kbps() else {
            return Ok(());
        };
        self.run(&cdb_set_cd_speed(kbps), &mut [], 0).map(|_| ())
    }

    fn capabilities(&self) -> Capabilities {
        self.capabilities
    }
}

/// A SCSI transport as a [`Device`]: the drive plus how to find where the OS mounted the disc.
pub(crate) struct ScsiDevice<T> {
    source: ScsiSource<T>,
    mount: Box<dyn Fn() -> Option<std::path::PathBuf> + Send>,
}

impl<T: ScsiTransport> ScsiDevice<T> {
    pub(crate) fn new(transport: T, mount: Box<dyn Fn() -> Option<std::path::PathBuf> + Send>) -> Self {
        ScsiDevice { source: ScsiSource::new(transport), mount }
    }
}

impl<T: ScsiTransport> SectorSource for ScsiDevice<T> {
    fn toc(&mut self) -> Result<Toc> {
        self.source.toc()
    }
    fn read_audio(&mut self, lba: u32, count: u16, pcm: &mut [u8], c2: Option<&mut [u8]>) -> Result<ReadFlags> {
        self.source.read_audio(lba, count, pcm, c2)
    }
    fn cd_text(&mut self) -> Result<Option<CdText>> {
        self.source.cd_text()
    }
    fn isrc(&mut self, track: u8) -> Result<Option<String>> {
        self.source.isrc(track)
    }
    fn mcn(&mut self) -> Result<Option<String>> {
        self.source.mcn()
    }
    fn set_speed(&mut self, speed: ReadSpeed) -> Result<()> {
        self.source.set_speed(speed)
    }
    fn capabilities(&self) -> Capabilities {
        self.source.capabilities()
    }
}

impl<T: ScsiTransport> Device for ScsiDevice<T> {
    fn prepare(&mut self) {
        self.source.prepare();
    }
    fn dvd_copyright(&mut self) -> Option<u8> {
        self.source.dvd_copyright()
    }
    fn mount_point(&self) -> Option<std::path::PathBuf> {
        (self.mount)()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SECTORS_PER_SECOND;
    use std::collections::VecDeque;

    /// Replays canned answers and records the commands.
    struct Script {
        answers: VecDeque<std::result::Result<Vec<u8>, (u8, u8, u8)>>,
        sent: Vec<Vec<u8>>,
    }

    impl ScsiTransport for Script {
        fn execute(&mut self, cdb: &[u8], data: &mut [u8]) -> std::result::Result<usize, ScsiError> {
            self.sent.push(cdb.to_vec());
            match self.answers.pop_front() {
                Some(Ok(bytes)) => {
                    let n = bytes.len().min(data.len());
                    data[..n].copy_from_slice(&bytes[..n]);
                    Ok(n)
                }
                Some(Err((key, asc, ascq))) => Err(ScsiError::Check { key, asc, ascq }),
                None => Ok(data.len()),
            }
        }
    }

    fn toc_bytes(rows: &[(u8, u8, u32)]) -> Vec<u8> {
        let mut out = vec![0, 0, 1, rows.len() as u8 - 1];
        for (number, control, lba) in rows {
            out.extend_from_slice(&[0, 0x10 | control, *number, 0]);
            out.extend_from_slice(&lba.to_be_bytes());
        }
        let len = (out.len() - 2) as u16;
        out[..2].copy_from_slice(&len.to_be_bytes());
        out
    }

    #[test]
    fn cdbs_have_the_right_bytes() {
        assert_eq!(cdb_read_toc(2, 804), [0x43, 0, 2, 0, 0, 0, 0, 0x03, 0x24, 0]);
        // READ CD, CD-DA, LBA 0x01020304, 27 sectors, user data + C2 bits.
        assert_eq!(cdb_read_cd(0x01020304, 27, true), [0xBE, 0x04, 1, 2, 3, 4, 0, 0, 27, 0x12, 0, 0]);
        assert_eq!(cdb_read_cd(0, 1, false)[9], 0x10);
        assert_eq!(cdb_read_sub_channel(0x03, 5), [0x42, 0, 0x40, 0x03, 0, 0, 5, 0, 24, 0]);
        assert_eq!(cdb_set_cd_speed(0xFFFF)[2..6], [0xFF, 0xFF, 0xFF, 0xFF]);
        assert_eq!(cdb_read_dvd_copyright()[..], [0xAD, 0, 0, 0, 0, 0, 0, 1, 0, 8, 0, 0]);
    }

    #[test]
    fn parses_a_toc_with_a_data_track() {
        let data = toc_bytes(&[(1, 0, 0), (2, 1, 20_000), (3, 4, 50_000), (0xAA, 4, 60_000)]);
        let toc = parse_toc(&data).unwrap();
        assert_eq!((toc.first, toc.last), (1, 3));
        assert!(toc.track(2).unwrap().pre_emphasis);
        assert!(!toc.track(3).unwrap().audio);
        assert_eq!(toc.leadout_lba, 60_000);
        assert_eq!(toc.length_ms(), u64::from((50_000 - 11_400) * 1000 / SECTORS_PER_SECOND));
    }

    #[test]
    fn a_toc_without_leadout_is_no_disc() {
        assert!(matches!(parse_toc(&toc_bytes(&[(1, 0, 0)])), Err(DiscError::NoDisc)));
        assert!(matches!(parse_toc(&[0, 2]), Err(DiscError::NoDisc)));
    }

    #[test]
    fn parses_a_full_toc_with_two_sessions() {
        let row = |session: u8, control: u8, point: u8, m: u8, s: u8, f: u8| {
            [session, 0x10 | control, 0, point, 0, 0, 0, 0, m, s, f]
        };
        // Track 1 at 00:02:00 (LBA 0), track 2 at 05:00:00 (LBA 22350), data track 3 in session 2.
        let mut data = vec![0, 0, 1, 2];
        for r in [
            row(1, 0, 0xA0, 1, 0, 0),
            row(1, 0, 0xA1, 2, 0, 0),
            row(1, 0, 0xA2, 9, 0, 0),
            row(1, 0, 1, 0, 2, 0),
            row(1, 0, 2, 5, 2, 0),
            row(2, 4, 3, 12, 0, 0),
            row(2, 4, 0xA2, 14, 0, 0),
        ] {
            data.extend_from_slice(&r);
        }
        let len = (data.len() - 2) as u16;
        data[..2].copy_from_slice(&len.to_be_bytes());
        let toc = parse_full_toc(&data).unwrap();
        assert_eq!(toc.track(1).unwrap().start_lba, 0);
        assert_eq!(toc.track(2).unwrap().start_lba, 22_500);
        assert!(!toc.track(3).unwrap().audio);
        assert_eq!(toc.leadout_lba, 14 * 60 * 75 - 150);
        assert_eq!(toc.audio_tracks().count(), 2);
    }

    #[test]
    fn codes_are_trimmed_and_checked() {
        assert_eq!(valid_code(b"USRC17607839\0").as_deref(), Some("USRC17607839"));
        assert_eq!(valid_code(b"0000000000000"), None);
        assert_eq!(valid_code(b"\0\0\0"), None);
        assert_eq!(valid_code(b"AB-123"), None);
    }

    #[test]
    fn parses_isrc_and_mcn() {
        let mut data = vec![0u8; 24];
        data[4] = 0x03;
        data[8] = 0x80;
        data[9..21].copy_from_slice(b"USRC17607839");
        assert_eq!(parse_isrc(&data).as_deref(), Some("USRC17607839"));
        data[8] = 0;
        assert_eq!(parse_isrc(&data), None, "TCVAL clear");
        let mut mcn = vec![0u8; 24];
        mcn[8] = 0x80;
        mcn[9..22].copy_from_slice(b"0000000000000");
        assert_eq!(parse_mcn(&mcn), None, "all zeros is 'none'");
        mcn[9..22].copy_from_slice(b"0724349691704");
        assert_eq!(parse_mcn(&mcn).as_deref(), Some("0724349691704"));
    }

    #[test]
    fn dvd_copyright_type() {
        assert_eq!(parse_dvd_copyright(&[0, 6, 0, 0, 1, 0x3F, 0, 0]), Some(1));
        assert_eq!(parse_dvd_copyright(&[0, 6]), None);
    }

    #[test]
    fn c2_is_probed_and_de_interleaved() {
        let toc = toc_bytes(&[(1, 0, 0), (0xAA, 0, 100)]);
        let mut sector = vec![7u8; SECTOR_BYTES];
        sector.extend(vec![1u8; C2_BYTES]);
        let script = Script {
            answers: VecDeque::from([Ok(toc), Ok(vec![0; SECTOR_BYTES + C2_BYTES]), Ok(sector)]),
            sent: vec![],
        };
        let mut source = ScsiSource::new(script);
        source.prepare();
        assert!(source.capabilities().c2);
        let (mut pcm, mut c2) = (vec![0u8; SECTOR_BYTES], vec![0u8; C2_BYTES]);
        let flags = source.read_audio(5, 1, &mut pcm, Some(&mut c2)).unwrap();
        assert!(flags.c2);
        assert!(pcm.iter().all(|b| *b == 7) && c2.iter().all(|b| *b == 1));
        // Without a C2 buffer the plain command (no C2 flag) is used.
        source.read_audio(5, 1, &mut pcm, None).unwrap();
        assert_eq!(source.transport.sent.last().unwrap()[9], 0x10);
    }

    #[test]
    fn sense_data_maps_to_humane_errors() {
        assert!(matches!(map_error(ScsiError::Check { key: 2, asc: 0x3A, ascq: 0 }, 0), DiscError::NoDisc));
        assert!(matches!(
            map_error(ScsiError::Check { key: 3, asc: 0x11, ascq: 5 }, 9),
            DiscError::Read { lba: 9, .. }
        ));
        let denied = ScsiError::Os(std::io::Error::from(ErrorKind::PermissionDenied));
        assert!(matches!(map_error(denied, 0), DiscError::Permission { .. }));
    }

    #[test]
    fn sense_formats_are_read() {
        let mut fixed = [0u8; 18];
        fixed[0] = 0x70;
        fixed[2] = 0x02;
        fixed[12] = 0x3A;
        assert!(matches!(check_condition(&fixed), ScsiError::Check { key: 2, asc: 0x3A, ascq: 0 }));
        assert!(matches!(check_condition(&[0x72, 0x03, 0x11, 0x05]), ScsiError::Check { key: 3, asc: 0x11, ascq: 5 }));
        assert!(matches!(check_condition(&[]), ScsiError::Check { key: 0, .. }));
    }

    #[test]
    fn cd_text_absent_is_none_not_an_error() {
        let toc = toc_bytes(&[(1, 0, 0), (0xAA, 0, 100)]);
        let script = Script { answers: VecDeque::from([Ok(toc), Ok(vec![0; 2400]), Err((5, 0x24, 0))]), sent: vec![] };
        let mut source = ScsiSource::new(script);
        assert_eq!(source.cd_text().unwrap(), None);
    }
}
