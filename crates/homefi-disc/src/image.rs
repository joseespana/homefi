//! Disc images as sector sources, for tests and mock discs: [`ImageSource`] serves audio from
//! memory or a raw BIN file, [`FlakySource`] wraps any source and scripts misbehaviour.
//! Public so the rip pipeline's tests (and a UI mock) use the same fixtures.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::cdtext::{build_packs, CdText};
use crate::error::{DiscError, Result};
use crate::source::{Capabilities, ReadFlags, ReadSpeed, SectorSource, C2_BYTES, SAMPLES_PER_SECTOR, SECTOR_BYTES};
use crate::toc::{RawTocEntry, Toc};

enum Backing {
    Memory(Vec<u8>),
    File(File),
}

/// A disc in a file or in memory: raw 2352-byte sectors, `lba` 0 first.
pub struct ImageSource {
    toc: Toc,
    backing: Backing,
    cd_text: Option<CdText>,
    isrcs: HashMap<u8, String>,
    mcn: Option<String>,
    capabilities: Capabilities,
}

impl ImageSource {
    fn new(toc: Toc, backing: Backing) -> Self {
        ImageSource {
            toc,
            backing,
            cd_text: None,
            isrcs: HashMap::new(),
            mcn: None,
            capabilities: Capabilities::default(),
        }
    }

    /// A synthetic audio disc: one track per entry (sector counts), deterministic stereo audio
    /// that differs from sector to sector and between channels, so any shift or swap shows.
    pub fn synthetic(track_sectors: &[u32]) -> Self {
        let mut entries = Vec::new();
        let mut start = 0u32;
        for (i, sectors) in track_sectors.iter().enumerate() {
            entries.push(RawTocEntry { number: i as u8 + 1, control: 0, start_lba: start });
            start += sectors;
        }
        let toc = Toc::from_entries(&entries, start);
        let mut bytes = Vec::with_capacity(start as usize * SECTOR_BYTES);
        for lba in 0..start {
            bytes.extend_from_slice(&Self::synthetic_sector(lba));
        }
        Self::new(toc, Backing::Memory(bytes))
    }

    /// The audio of sector `lba` in a synthetic image.
    pub fn synthetic_sector(lba: u32) -> [u8; SECTOR_BYTES] {
        let mut out = [0u8; SECTOR_BYTES];
        for frame in 0..SAMPLES_PER_SECTOR / 2 {
            let n = u64::from(lba) * (SAMPLES_PER_SECTOR as u64 / 2) + frame as u64;
            // Cheap hash: every frame distinct, no silence runs (which would hide shifts).
            let mixed = n.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 33;
            let left = (mixed as i32 & 0x7FFF) as i16 - 0x4000;
            let right = ((mixed >> 15) as i32 & 0x7FFF) as i16 - 0x4000;
            out[frame * 4..frame * 4 + 2].copy_from_slice(&left.to_le_bytes());
            out[frame * 4 + 2..frame * 4 + 4].copy_from_slice(&right.to_le_bytes());
        }
        out
    }

    /// Raw audio from a `.bin` file laid out as described by `toc`.
    pub fn from_bin(path: &Path, toc: Toc) -> Result<Self> {
        Ok(Self::new(toc, Backing::File(File::open(path)?)))
    }

    /// Raw audio in memory laid out as described by `toc`.
    pub fn from_bytes(bytes: Vec<u8>, toc: Toc) -> Self {
        Self::new(toc, Backing::Memory(bytes))
    }

    pub fn with_toc(mut self, toc: Toc) -> Self {
        self.toc = toc;
        self
    }
    pub fn with_cd_text(mut self, text: CdText) -> Self {
        self.cd_text = Some(text);
        self.capabilities.cd_text = true;
        self
    }
    pub fn with_isrc(mut self, track: u8, isrc: &str) -> Self {
        self.isrcs.insert(track, isrc.to_owned());
        self
    }
    pub fn with_mcn(mut self, mcn: &str) -> Self {
        self.mcn = Some(mcn.to_owned());
        self
    }
    /// Pretend the drive reports C2 (all clear, unless a wrapper says otherwise).
    pub fn with_c2(mut self, on: bool) -> Self {
        self.capabilities.c2 = on;
        self
    }
}

impl SectorSource for ImageSource {
    fn toc(&mut self) -> Result<Toc> {
        Ok(self.toc.clone())
    }

    fn read_audio(&mut self, lba: u32, count: u16, pcm: &mut [u8], c2: Option<&mut [u8]>) -> Result<ReadFlags> {
        let bytes = usize::from(count) * SECTOR_BYTES;
        assert!(pcm.len() >= bytes, "pcm buffer too small");
        let outside = u64::from(lba) + u64::from(count) > u64::from(self.toc.leadout_lba);
        if outside {
            return Err(DiscError::Read { lba, message: "beyond the end of the image".into() });
        }
        let at = u64::from(lba) * SECTOR_BYTES as u64;
        match &mut self.backing {
            Backing::Memory(data) => {
                let from = at as usize;
                let slice = data
                    .get(from..from + bytes)
                    .ok_or_else(|| DiscError::Read { lba, message: "short image".into() })?;
                pcm[..bytes].copy_from_slice(slice);
            }
            Backing::File(file) => {
                file.seek(SeekFrom::Start(at))?;
                file.read_exact(&mut pcm[..bytes])?;
            }
        }
        match c2 {
            Some(c2) if self.capabilities.c2 => {
                c2[..usize::from(count) * C2_BYTES].fill(0);
                Ok(ReadFlags { c2: true })
            }
            _ => Ok(ReadFlags::default()),
        }
    }

    fn cd_text(&mut self) -> Result<Option<CdText>> {
        Ok(self.cd_text.clone())
    }
    fn isrc(&mut self, track: u8) -> Result<Option<String>> {
        Ok(self.isrcs.get(&track).cloned())
    }
    fn mcn(&mut self) -> Result<Option<String>> {
        Ok(self.mcn.clone())
    }
    fn set_speed(&mut self, _speed: ReadSpeed) -> Result<()> {
        Ok(())
    }
    fn capabilities(&self) -> Capabilities {
        self.capabilities
    }
}

impl ImageSource {
    /// The CD-Text of the image as raw packs (what a drive would return), for transport tests.
    pub fn cd_text_packs(&self) -> Option<Vec<u8>> {
        self.cd_text.as_ref().map(build_packs)
    }
}

/// What goes wrong with a sector, per read ordinal (0 = the first time that sector is read).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fault {
    /// Audio is replaced by bytes that differ on every read.
    Garbage,
    /// The read fails.
    Error,
    /// The audio is right but the C2 bits are set.
    C2,
}

/// Wraps a source and breaks chosen sectors on chosen reads, for the secure-reader tests.
pub struct FlakySource<S> {
    inner: S,
    /// (lba, read ordinal) -> fault; ordinal `None` = every read.
    faults: Vec<(u32, Option<u32>, Fault)>,
    reads: HashMap<u32, u32>,
    log: Vec<(u32, u16)>,
}

impl<S: SectorSource> FlakySource<S> {
    pub fn new(inner: S) -> Self {
        FlakySource { inner, faults: Vec::new(), reads: HashMap::new(), log: Vec::new() }
    }
    /// The `nth` (0-based) read of sector `lba` returns garbage.
    pub fn garbage_on(mut self, lba: u32, nth: u32) -> Self {
        self.faults.push((lba, Some(nth), Fault::Garbage));
        self
    }
    /// Every read of the sector returns different garbage: it never agrees with itself.
    pub fn always_garbage(mut self, lba: u32) -> Self {
        self.faults.push((lba, None, Fault::Garbage));
        self
    }
    pub fn error_on(mut self, lba: u32, nth: u32) -> Self {
        self.faults.push((lba, Some(nth), Fault::Error));
        self
    }
    pub fn always_error(mut self, lba: u32) -> Self {
        self.faults.push((lba, None, Fault::Error));
        self
    }
    /// The `nth` read reports C2 errors on the sector (audio intact).
    pub fn c2_on(mut self, lba: u32, nth: u32) -> Self {
        self.faults.push((lba, Some(nth), Fault::C2));
        self
    }
    pub fn always_c2(mut self, lba: u32) -> Self {
        self.faults.push((lba, None, Fault::C2));
        self
    }
    /// How many times sector `lba` has been read.
    pub fn reads_of(&self, lba: u32) -> u32 {
        self.reads.get(&lba).copied().unwrap_or(0)
    }
    /// Every `(lba, count)` request, in order.
    pub fn requests(&self) -> &[(u32, u16)] {
        &self.log
    }
    pub fn total_reads(&self) -> u32 {
        self.reads.values().sum()
    }

    fn fault_for(&self, lba: u32, ordinal: u32) -> Option<Fault> {
        self.faults.iter().find(|(l, n, _)| *l == lba && n.is_none_or(|n| n == ordinal)).map(|(_, _, f)| *f)
    }
}

impl<S: SectorSource> SectorSource for FlakySource<S> {
    fn toc(&mut self) -> Result<Toc> {
        self.inner.toc()
    }

    fn read_audio(&mut self, lba: u32, count: u16, pcm: &mut [u8], mut c2: Option<&mut [u8]>) -> Result<ReadFlags> {
        self.log.push((lba, count));
        let flags = self.inner.read_audio(lba, count, pcm, c2.as_deref_mut())?;
        let mut failed = false;
        for i in 0..u32::from(count) {
            let sector = lba + i;
            let ordinal = self.reads.entry(sector).or_insert(0);
            let this = *ordinal;
            *ordinal += 1;
            let from = i as usize * SECTOR_BYTES;
            match self.fault_for(sector, this) {
                Some(Fault::Garbage) => {
                    for (k, byte) in pcm[from..from + SECTOR_BYTES].iter_mut().enumerate() {
                        *byte = (k as u32).wrapping_mul(31).wrapping_add(this.wrapping_mul(7919) + 13) as u8;
                    }
                }
                Some(Fault::Error) => failed = true,
                Some(Fault::C2) => {
                    if let (Some(c2), true) = (c2.as_deref_mut(), flags.c2) {
                        c2[i as usize * C2_BYTES..(i as usize + 1) * C2_BYTES].fill(0x01);
                    }
                }
                None => {}
            }
        }
        if failed {
            return Err(DiscError::Read { lba, message: "scripted read error".into() });
        }
        Ok(flags)
    }

    fn cd_text(&mut self) -> Result<Option<CdText>> {
        self.inner.cd_text()
    }
    fn isrc(&mut self, track: u8) -> Result<Option<String>> {
        self.inner.isrc(track)
    }
    fn mcn(&mut self) -> Result<Option<String>> {
        self.inner.mcn()
    }
    fn set_speed(&mut self, speed: ReadSpeed) -> Result<()> {
        self.inner.set_speed(speed)
    }
    fn capabilities(&self) -> Capabilities {
        self.inner.capabilities()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_image_serves_its_sectors() {
        let mut image = ImageSource::synthetic(&[10, 20]);
        let toc = image.toc().unwrap();
        assert_eq!(toc.leadout_lba, 30);
        assert_eq!(toc.track(2).unwrap().start_lba, 10);
        let mut pcm = vec![0u8; SECTOR_BYTES * 2];
        image.read_audio(9, 2, &mut pcm, None).unwrap();
        assert_eq!(&pcm[..SECTOR_BYTES], &ImageSource::synthetic_sector(9));
        assert_eq!(&pcm[SECTOR_BYTES..], &ImageSource::synthetic_sector(10));
        assert!(image.read_audio(29, 2, &mut pcm, None).is_err());
    }

    #[test]
    fn flaky_garbage_differs_per_read() {
        let mut flaky = FlakySource::new(ImageSource::synthetic(&[5])).always_garbage(2);
        let (mut a, mut b) = (vec![0u8; SECTOR_BYTES], vec![0u8; SECTOR_BYTES]);
        flaky.read_audio(2, 1, &mut a, None).unwrap();
        flaky.read_audio(2, 1, &mut b, None).unwrap();
        assert_ne!(a, b);
        assert_eq!(flaky.reads_of(2), 2);
    }

    #[test]
    fn bin_file_matches_memory() {
        let memory = ImageSource::synthetic(&[4]);
        let toc = ImageSource::synthetic(&[4]).toc().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("disc.bin");
        let bytes: Vec<u8> = (0..4).flat_map(ImageSource::synthetic_sector).collect();
        std::fs::write(&path, bytes).unwrap();
        let mut file = ImageSource::from_bin(&path, toc).unwrap();
        let (mut a, mut b) = (vec![0u8; SECTOR_BYTES], vec![0u8; SECTOR_BYTES]);
        file.read_audio(3, 1, &mut a, None).unwrap();
        let mut memory = memory;
        memory.read_audio(3, 1, &mut b, None).unwrap();
        assert_eq!(a, b);
    }
}
