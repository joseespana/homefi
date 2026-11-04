//! macOS transport: the `DKIOCCD*` ioctls Apple's `IOCDMediaBSDClient` exposes on the raw
//! device (`/dev/rdiskN`). Struct layouts and request numbers come from
//! `<IOKit/storage/IOCDMediaBSDClient.h>`, `<IOKit/storage/IODVDMediaBSDClient.h>` and
//! `<sys/disk.h>`; the unit tests pin sizes and numbers to the values the C compiler gives.
//!
//! No root needed when the console user owns the device node, which macOS arranges for
//! removable media it mounts. If the kernel refuses (`EACCES`, `EBUSY`) the error says so and
//! nothing is forced.

use std::collections::HashMap;
use std::ffi::{c_int, c_ulong, c_void, CStr};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use super::Candidate;
use crate::cdtext::{parse_cd_text, CdText};
use crate::drives::{Device, DriveId};
use crate::error::{DiscError, Result};
use crate::scsi::{parse_dvd_copyright, parse_full_toc, valid_code};
use crate::source::{Capabilities, ReadFlags, ReadSpeed, SectorSource, C2_BYTES, CHUNK_SECTORS, SECTOR_BYTES};
use crate::toc::Toc;

// ---- ioctl numbers (BSD _IOC encoding) --------------------------------------------------------

const IOC_OUT: c_ulong = 0x4000_0000;
const IOC_IN: c_ulong = 0x8000_0000;
const IOC_PARAM_MASK: c_ulong = 0x1FFF;

const fn ioc(direction: c_ulong, group: u8, number: u8, size: usize) -> c_ulong {
    direction | ((size as c_ulong & IOC_PARAM_MASK) << 16) | ((group as c_ulong) << 8) | number as c_ulong
}

const DKIOCCDREAD: c_ulong = ioc(IOC_IN | IOC_OUT, b'd', 96, std::mem::size_of::<DkCdRead>());
const DKIOCCDREADISRC: c_ulong = ioc(IOC_IN | IOC_OUT, b'd', 97, std::mem::size_of::<DkCdReadIsrc>());
const DKIOCCDREADMCN: c_ulong = ioc(IOC_IN | IOC_OUT, b'd', 98, std::mem::size_of::<DkCdReadMcn>());
const DKIOCCDSETSPEED: c_ulong = ioc(IOC_IN, b'd', 99, std::mem::size_of::<u16>());
const DKIOCCDREADTOC: c_ulong = ioc(IOC_IN | IOC_OUT, b'd', 100, std::mem::size_of::<DkCdReadToc>());
const DKIOCDVDREADSTRUCTURE: c_ulong = ioc(IOC_IN, b'd', 128, std::mem::size_of::<DkDvdReadStructure>());

// ---- <IOKit/storage/IOCDTypes.h> ----------------------------------------------------------------

const SECTOR_AREA_ERROR_FLAGS: u8 = 0x02;
const SECTOR_AREA_USER: u8 = 0x10;
const SECTOR_TYPE_CDDA: u8 = 0x01;
const TOC_FORMAT_FULL: u8 = 0x02;
const TOC_FORMAT_TEXT: u8 = 0x05;
const DVD_STRUCTURE_COPYRIGHT: u8 = 0x01;
/// The answer buffers: a full TOC is under 1 KiB; CD-Text rarely passes 4 KiB.
const TOC_BUFFER: usize = 4096;
const TEXT_BUFFER: usize = 16 * 1024;

#[repr(C)]
struct DkCdRead {
    offset: u64,
    sector_area: u8,
    sector_type: u8,
    reserved: [u8; 10],
    buffer_length: u32,
    buffer: *mut c_void,
}

#[repr(C)]
struct DkCdReadToc {
    format: u8,
    format_as_time: u8,
    reserved0: [u8; 5],
    address: u8,
    reserved1: [u8; 6],
    buffer_length: u16,
    buffer: *mut c_void,
}

#[repr(C)]
struct DkCdReadIsrc {
    isrc: [u8; 13],
    track: u8,
    reserved: [u8; 2],
}

#[repr(C)]
struct DkCdReadMcn {
    mcn: [u8; 14],
    reserved: [u8; 2],
}

#[repr(C)]
struct DkDvdReadStructure {
    format: u8,
    reserved0: [u8; 3],
    address: u32,
    grant_id: u8,
    layer: u8,
    reserved1: [u8; 4],
    buffer_length: u16,
    buffer: *mut c_void,
}

fn ioctl<T>(fd: RawFd, request: c_ulong, argument: &mut T) -> std::io::Result<()> {
    // SAFETY: `fd` is an open descriptor kept alive by the caller's `OwnedFd`; `argument` is an
    // exclusively borrowed `repr(C)` struct whose layout is the one `request` expects (pinned by
    // the unit tests); any buffer pointer inside it points into a slice that outlives this call
    // and is at least `buffer_length` bytes long.
    let result = unsafe { libc::ioctl(fd, request, argument as *mut T) };
    if result == -1 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn map_errno(error: &std::io::Error, lba: u32) -> DiscError {
    match error.raw_os_error() {
        Some(libc::EACCES | libc::EPERM) => DiscError::Permission {
            hint: "If another app (Music, a ripper) is using the disc, close it and try again.".into(),
        },
        Some(libc::EBUSY) => DiscError::Busy,
        Some(libc::ENXIO | libc::ENODEV | libc::ENOENT) => DiscError::NoDisc,
        _ => DiscError::Read { lba, message: error.to_string() },
    }
}

// ---- device ------------------------------------------------------------------------------------

struct MacDevice {
    fd: OwnedFd,
    id: String,
    capabilities: Capabilities,
    /// Audio + C2 interleaved as the kernel returns it; sized once for the largest window.
    interleaved: Vec<u8>,
}

fn device_path(id: &DriveId) -> Result<String> {
    let digits = id.0.strip_prefix("disk").unwrap_or("");
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(DiscError::NoDrive);
    }
    Ok(format!("/dev/r{}", id.0))
}

fn open_fd(path: &str) -> Result<OwnedFd> {
    let c_path = std::ffi::CString::new(path).map_err(|_| DiscError::NoDrive)?;
    // SAFETY: `c_path` is a valid NUL-terminated string; the flags request a read-only,
    // non-blocking open (non-blocking so an empty or spinning-up drive does not hang us).
    let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDONLY | libc::O_NONBLOCK) };
    if fd < 0 {
        return Err(map_errno(&std::io::Error::last_os_error(), 0));
    }
    // SAFETY: `fd` was just returned by `open`, is valid and owned by nobody else.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

pub(crate) fn open(id: &DriveId) -> Result<Box<dyn Device>> {
    let fd = open_fd(&device_path(id)?)?;
    let device = MacDevice {
        fd,
        id: id.0.clone(),
        capabilities: Capabilities::default(),
        interleaved: vec![0; usize::from(CHUNK_SECTORS) * (SECTOR_BYTES + C2_BYTES)],
    };
    Ok(Box::new(device))
}

impl MacDevice {
    fn raw(&self) -> RawFd {
        self.fd.as_raw_fd()
    }

    fn read_toc(&self, format: u8, buffer: &mut [u8]) -> std::io::Result<usize> {
        let mut request = DkCdReadToc {
            format,
            format_as_time: 1,
            reserved0: [0; 5],
            address: 0,
            reserved1: [0; 6],
            buffer_length: buffer.len().min(usize::from(u16::MAX)) as u16,
            buffer: buffer.as_mut_ptr().cast(),
        };
        ioctl(self.raw(), DKIOCCDREADTOC, &mut request)?;
        Ok(usize::from(request.buffer_length).min(buffer.len()))
    }

    fn read_raw(&self, lba: u32, count: u16, area: u8, block: usize, buffer: &mut [u8]) -> Result<()> {
        let wanted = usize::from(count) * block;
        let mut request = DkCdRead {
            offset: u64::from(lba) * block as u64,
            sector_area: area,
            sector_type: SECTOR_TYPE_CDDA,
            reserved: [0; 10],
            buffer_length: wanted as u32,
            buffer: buffer[..wanted].as_mut_ptr().cast(),
        };
        ioctl(self.raw(), DKIOCCDREAD, &mut request).map_err(|e| map_errno(&e, lba))?;
        if (request.buffer_length as usize) < wanted {
            return Err(DiscError::Read { lba, message: "short read".into() });
        }
        Ok(())
    }

    /// C2 counts as supported when a read asking for it succeeds and its audio equals a plain
    /// read of the same sector. This also proves the byte offset of the C2 layout is right.
    fn probe_c2(&mut self, toc: &Toc) -> bool {
        let Some(track) = toc.audio_tracks().find(|t| t.sectors() > 1) else {
            return false;
        };
        let lba = track.start_lba + 1;
        let stride = SECTOR_BYTES + C2_BYTES;
        let (mut plain_a, mut plain_b) = (vec![0u8; SECTOR_BYTES], vec![0u8; SECTOR_BYTES]);
        let mut with_c2 = vec![0u8; stride];
        let area = SECTOR_AREA_USER | SECTOR_AREA_ERROR_FLAGS;
        self.read_raw(lba, 1, SECTOR_AREA_USER, SECTOR_BYTES, &mut plain_a).is_ok()
            && self.read_raw(lba, 1, SECTOR_AREA_USER, SECTOR_BYTES, &mut plain_b).is_ok()
            && plain_a == plain_b
            && self.read_raw(lba, 1, area, stride, &mut with_c2).is_ok()
            && with_c2[..SECTOR_BYTES] == plain_a[..]
    }
}

impl SectorSource for MacDevice {
    fn toc(&mut self) -> Result<Toc> {
        let mut buffer = vec![0u8; TOC_BUFFER];
        let got = self.read_toc(TOC_FORMAT_FULL, &mut buffer).map_err(|e| map_errno(&e, 0))?;
        parse_full_toc(&buffer[..got])
    }

    fn read_audio(&mut self, lba: u32, count: u16, pcm: &mut [u8], c2: Option<&mut [u8]>) -> Result<ReadFlags> {
        let sectors = usize::from(count);
        assert!(pcm.len() >= sectors * SECTOR_BYTES, "pcm buffer too small");
        match c2 {
            Some(c2) if self.capabilities.c2 && sectors <= usize::from(CHUNK_SECTORS) => {
                let stride = SECTOR_BYTES + C2_BYTES;
                let mut buffer = std::mem::take(&mut self.interleaved);
                let result = self.read_raw(lba, count, SECTOR_AREA_USER | SECTOR_AREA_ERROR_FLAGS, stride, &mut buffer);
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
                result.map(|()| ReadFlags { c2: true })
            }
            _ => {
                self.read_raw(lba, count, SECTOR_AREA_USER, SECTOR_BYTES, &mut pcm[..sectors * SECTOR_BYTES])?;
                Ok(ReadFlags::default())
            }
        }
    }

    fn cd_text(&mut self) -> Result<Option<CdText>> {
        let mut buffer = vec![0u8; TEXT_BUFFER];
        let Ok(got) = self.read_toc(TOC_FORMAT_TEXT, &mut buffer) else {
            return Ok(None);
        };
        Ok(parse_cd_text(&buffer[..got]))
    }

    fn isrc(&mut self, track: u8) -> Result<Option<String>> {
        let mut request = DkCdReadIsrc { isrc: [0; 13], track, reserved: [0; 2] };
        if ioctl(self.raw(), DKIOCCDREADISRC, &mut request).is_err() {
            return Ok(None);
        }
        Ok(valid_code(&request.isrc))
    }

    fn mcn(&mut self) -> Result<Option<String>> {
        let mut request = DkCdReadMcn { mcn: [0; 14], reserved: [0; 2] };
        if ioctl(self.raw(), DKIOCCDREADMCN, &mut request).is_err() {
            return Ok(None);
        }
        Ok(valid_code(&request.mcn))
    }

    fn set_speed(&mut self, speed: ReadSpeed) -> Result<()> {
        let Some(mut kbps) = speed.kbps() else {
            return Ok(());
        };
        ioctl(self.raw(), DKIOCCDSETSPEED, &mut kbps).map_err(|e| map_errno(&e, 0))
    }

    fn capabilities(&self) -> Capabilities {
        self.capabilities
    }
}

impl Device for MacDevice {
    fn prepare(&mut self) {
        if let Ok(toc) = self.toc() {
            self.capabilities.c2 = self.probe_c2(&toc);
            self.capabilities.cd_text = true;
        }
    }

    fn dvd_copyright(&mut self) -> Option<u8> {
        let mut answer = [0u8; 8];
        let mut request = DkDvdReadStructure {
            format: DVD_STRUCTURE_COPYRIGHT,
            reserved0: [0; 3],
            address: 0,
            grant_id: 0,
            layer: 0,
            reserved1: [0; 4],
            buffer_length: answer.len() as u16,
            buffer: answer.as_mut_ptr().cast(),
        };
        ioctl(self.raw(), DKIOCDVDREADSTRUCTURE, &mut request).ok()?;
        parse_dvd_copyright(&answer)
    }

    fn mount_point(&self) -> Option<PathBuf> {
        mounts().into_iter().find(|m| m.disk == self.id).map(|m| m.on)
    }
}

// ---- discovery -----------------------------------------------------------------------------------

struct Mount {
    /// `disk4` for `/dev/disk4s1`.
    disk: String,
    fs: String,
    on: PathBuf,
}

/// `disk4` out of `/dev/disk4s1`; `None` for anything that is not a disk node.
fn whole_disk(device: &str) -> Option<String> {
    let name = device.strip_prefix("/dev/")?.strip_prefix('r').unwrap_or_else(|| &device["/dev/".len()..]);
    let digits: String = name.strip_prefix("disk")?.chars().take_while(char::is_ascii_digit).collect();
    (!digits.is_empty()).then(|| format!("disk{digits}"))
}

fn c_str(chars: &[std::ffi::c_char]) -> String {
    // SAFETY: the kernel fills these fixed arrays with NUL-terminated strings, and `chars` is a
    // slice of such an array, so a NUL exists within its bounds.
    unsafe { CStr::from_ptr(chars.as_ptr()) }.to_string_lossy().into_owned()
}

fn mounts() -> Vec<Mount> {
    // SAFETY: a null buffer with size 0 only asks how many file systems are mounted.
    let count = unsafe { libc::getfsstat(std::ptr::null_mut(), 0, libc::MNT_NOWAIT) };
    if count <= 0 {
        return Vec::new();
    }
    let mut table: Vec<libc::statfs> = Vec::with_capacity(count as usize);
    let bytes = count as usize * std::mem::size_of::<libc::statfs>();
    // SAFETY: `table` has room for `count` entries, which is the size passed; the kernel writes
    // at most that many and returns how many it wrote, which is what `set_len` is given.
    let written = unsafe { libc::getfsstat(table.as_mut_ptr(), bytes as c_int, libc::MNT_NOWAIT) };
    if written <= 0 || written > count {
        return Vec::new();
    }
    // SAFETY: the first `written` entries were initialised by `getfsstat`.
    unsafe { table.set_len(written as usize) };
    table
        .iter()
        .filter_map(|entry| {
            let disk = whole_disk(&c_str(&entry.f_mntfromname))?;
            Some(Mount { disk, fs: c_str(&entry.f_fstypename), on: PathBuf::from(c_str(&entry.f_mntonname)) })
        })
        .collect()
}

/// File systems optical media mount as (audio CDs use `cddafs`).
fn is_optical_fs(fs: &str) -> bool {
    matches!(fs, "cddafs" | "cd9660" | "udf")
}

static NAMES: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

/// "Vendor Model" of a disk from `diskutil`, asked once per disk.
fn drive_name(disk: &str) -> String {
    let mut cache = NAMES.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let names = cache.get_or_insert_with(HashMap::new);
    if let Some(name) = names.get(disk) {
        return name.clone();
    }
    let name = Command::new("/usr/sbin/diskutil")
        .args(["info", &format!("/dev/{disk}")])
        .output()
        .ok()
        .and_then(|out| {
            String::from_utf8_lossy(&out.stdout).lines().find_map(|line| {
                let (key, value) = line.split_once(':')?;
                (key.trim() == "Device / Media Name").then(|| value.trim().trim_end_matches(" Media").to_owned())
            })
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Disc drive".to_owned());
    names.insert(disk.to_owned(), name.clone());
    name
}

/// Whole disks that answer CD or DVD ioctls, plus any disk mounted with an optical file system.
pub(crate) fn candidates() -> Result<Vec<Candidate>> {
    let mut disks: Vec<String> = mounts().into_iter().filter(|m| is_optical_fs(&m.fs)).map(|m| m.disk).collect();
    if let Ok(entries) = std::fs::read_dir("/dev") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(number) = name.strip_prefix("rdisk") else { continue };
            if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            let disk = format!("disk{number}");
            if disks.contains(&disk) {
                continue;
            }
            // Cheap and harmless: a TOC or DVD-structure request that non-optical disks refuse.
            if let Ok(mut device) = open(&DriveId(disk.clone())) {
                if device.toc().is_ok() || device.dvd_copyright().is_some() {
                    disks.push(disk);
                }
            }
        }
    }
    disks.sort();
    disks.dedup();
    Ok(disks.into_iter().map(|disk| Candidate { name: drive_name(&disk), id: DriveId(disk) }).collect())
}

/// `diskutil eject` unmounts whatever macOS mounted (cddafs, UDF) and then ejects.
pub(crate) fn eject(id: &DriveId) -> Result<()> {
    device_path(id)?;
    let status = Command::new("/usr/sbin/diskutil").args(["eject", &format!("/dev/{}", id.0)]).output()?.status;
    if status.success() {
        Ok(())
    } else {
        Err(DiscError::Busy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    /// Values printed by a C program built against the macOS SDK headers.
    #[test]
    fn layouts_and_request_numbers_match_the_sdk() {
        assert_eq!(size_of::<DkCdRead>(), 32);
        assert_eq!(offset_of!(DkCdRead, buffer_length), 20);
        assert_eq!(offset_of!(DkCdRead, buffer), 24);
        assert_eq!(size_of::<DkCdReadToc>(), 24);
        assert_eq!(offset_of!(DkCdReadToc, address), 7);
        assert_eq!(offset_of!(DkCdReadToc, buffer_length), 14);
        assert_eq!(offset_of!(DkCdReadToc, buffer), 16);
        assert_eq!(size_of::<DkCdReadIsrc>(), 16);
        assert_eq!(offset_of!(DkCdReadIsrc, track), 13);
        assert_eq!(size_of::<DkCdReadMcn>(), 16);
        assert_eq!(size_of::<DkDvdReadStructure>(), 24);
        assert_eq!(offset_of!(DkDvdReadStructure, layer), 9);
        assert_eq!(offset_of!(DkDvdReadStructure, buffer_length), 14);
        assert_eq!(offset_of!(DkDvdReadStructure, buffer), 16);
        assert_eq!(DKIOCCDREAD, 0xc020_6460);
        assert_eq!(DKIOCCDREADISRC, 0xc010_6461);
        assert_eq!(DKIOCCDREADMCN, 0xc010_6462);
        assert_eq!(DKIOCCDSETSPEED, 0x8002_6463);
        assert_eq!(DKIOCCDREADTOC, 0xc018_6464);
        assert_eq!(DKIOCDVDREADSTRUCTURE, 0x8018_6480);
    }

    #[test]
    fn disk_names_are_normalised() {
        assert_eq!(whole_disk("/dev/disk4s1").as_deref(), Some("disk4"));
        assert_eq!(whole_disk("/dev/rdisk12").as_deref(), Some("disk12"));
        assert_eq!(whole_disk("map auto_home"), None);
        assert_eq!(whole_disk("devfs"), None);
    }

    #[test]
    fn only_disk_ids_are_accepted() {
        assert_eq!(device_path(&DriveId("disk4".into())).unwrap(), "/dev/rdisk4");
        assert!(device_path(&DriveId("../etc/passwd".into())).is_err());
        assert!(device_path(&DriveId("disk".into())).is_err());
        assert!(device_path(&DriveId("disk4s1".into())).is_err());
    }

    #[test]
    fn listing_does_not_fail_without_a_drive() {
        assert!(candidates().is_ok());
    }
}
