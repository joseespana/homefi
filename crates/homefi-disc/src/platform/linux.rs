//! Linux transport: `SG_IO` on `/dev/srN` carrying the MMC commands of `scsi.rs`.
//!
//! Permission: desktop distributions grant the seated user access to optical drives through a
//! udev ACL; elsewhere the user must be in the `cdrom` group. The error says so.

use std::ffi::{c_int, c_ulong, c_void, CString};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::PathBuf;

use super::Candidate;
use crate::drives::{Device, DriveId};
use crate::error::{DiscError, Result};
use crate::scsi::{check_condition, ScsiDevice, ScsiError, ScsiTransport};

/// `<scsi/sg.h>`
const SG_IO: c_ulong = 0x2285;
const SG_DXFER_NONE: c_int = -1;
const SG_DXFER_FROM_DEV: c_int = -3;
/// `<linux/cdrom.h>`
const CDROMEJECT: c_ulong = 0x5309;
const SCSI_STATUS_CHECK_CONDITION: u8 = 0x02;
const SENSE_BYTES: usize = 32;
/// Reads can wait for a drive retrying a bad sector; give each command this long.
const COMMAND_TIMEOUT_MS: u32 = 60_000;
const CDB_MAX: usize = 16;

/// `struct sg_io_hdr` from `<scsi/sg.h>`.
#[repr(C)]
struct SgIoHdr {
    interface_id: c_int,
    dxfer_direction: c_int,
    cmd_len: u8,
    mx_sb_len: u8,
    iovec_count: u16,
    dxfer_len: u32,
    dxferp: *mut c_void,
    cmdp: *const u8,
    sbp: *mut u8,
    timeout: u32,
    flags: u32,
    pack_id: c_int,
    usr_ptr: *mut c_void,
    status: u8,
    masked_status: u8,
    msg_status: u8,
    sb_len_wr: u8,
    host_status: u16,
    driver_status: u16,
    resid: c_int,
    duration: u32,
    info: u32,
}

struct SgIo {
    fd: OwnedFd,
}

impl ScsiTransport for SgIo {
    fn execute(&mut self, cdb: &[u8], data: &mut [u8]) -> std::result::Result<usize, ScsiError> {
        if cdb.len() > CDB_MAX {
            return Err(ScsiError::Os(std::io::Error::from(std::io::ErrorKind::InvalidInput)));
        }
        let mut sense = [0u8; SENSE_BYTES];
        let mut header = SgIoHdr {
            interface_id: i32::from(b'S'),
            dxfer_direction: if data.is_empty() { SG_DXFER_NONE } else { SG_DXFER_FROM_DEV },
            cmd_len: cdb.len() as u8,
            mx_sb_len: SENSE_BYTES as u8,
            iovec_count: 0,
            dxfer_len: data.len() as u32,
            dxferp: if data.is_empty() { std::ptr::null_mut() } else { data.as_mut_ptr().cast() },
            cmdp: cdb.as_ptr(),
            sbp: sense.as_mut_ptr(),
            timeout: COMMAND_TIMEOUT_MS,
            flags: 0,
            pack_id: 0,
            usr_ptr: std::ptr::null_mut(),
            status: 0,
            masked_status: 0,
            msg_status: 0,
            sb_len_wr: 0,
            host_status: 0,
            driver_status: 0,
            resid: 0,
            duration: 0,
            info: 0,
        };
        // SAFETY: the descriptor is open for the life of `self`; `header` is a `repr(C)` mirror of
        // `sg_io_hdr` whose pointers (`cdb`, `data`, `sense`) stay valid and unmoved for the
        // whole call, with the lengths given in the header; the kernel writes only within them.
        let result = unsafe { libc::ioctl(self.fd.as_raw_fd(), SG_IO as _, &mut header as *mut SgIoHdr) };
        if result == -1 {
            return Err(ScsiError::Os(std::io::Error::last_os_error()));
        }
        if header.status == SCSI_STATUS_CHECK_CONDITION {
            return Err(check_condition(&sense[..usize::from(header.sb_len_wr).min(SENSE_BYTES)]));
        }
        if header.status != 0 || header.host_status != 0 {
            return Err(ScsiError::Os(std::io::Error::other(format!(
                "SCSI status {:#x}, host {:#x}",
                header.status, header.host_status
            ))));
        }
        Ok(data.len().saturating_sub(header.resid.max(0) as usize))
    }
}

fn device_path(id: &DriveId) -> Result<CString> {
    let digits = id.0.strip_prefix("/dev/sr").unwrap_or("");
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(DiscError::NoDrive);
    }
    CString::new(id.0.as_str()).map_err(|_| DiscError::NoDrive)
}

fn open_fd(id: &DriveId) -> Result<OwnedFd> {
    let path = device_path(id)?;
    // SAFETY: `path` is a valid NUL-terminated string; read-only, non-blocking (an empty drive
    // must not make `open` wait), close-on-exec.
    let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDONLY | libc::O_NONBLOCK | libc::O_CLOEXEC) };
    if fd < 0 {
        let error = std::io::Error::last_os_error();
        return Err(match error.raw_os_error() {
            Some(libc::EACCES | libc::EPERM) => {
                DiscError::Permission { hint: "Add your user to the `cdrom` group, then log out and back in.".into() }
            }
            Some(libc::EBUSY) => DiscError::Busy,
            Some(libc::ENOENT | libc::ENXIO | libc::ENODEV) => DiscError::NoDrive,
            _ => DiscError::Io(error),
        });
    }
    // SAFETY: `fd` was just returned by `open` and is owned by nobody else.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

pub(crate) fn open(id: &DriveId) -> Result<Box<dyn Device>> {
    let fd = open_fd(id)?;
    let device_name = id.0.clone();
    Ok(Box::new(ScsiDevice::new(SgIo { fd }, Box::new(move || mount_of(&device_name)))))
}

/// Mount point of `device` according to `/proc/mounts`.
fn mount_of(device: &str) -> Option<PathBuf> {
    parse_mounts(&std::fs::read_to_string("/proc/mounts").ok()?, device)
}

fn parse_mounts(table: &str, device: &str) -> Option<PathBuf> {
    table.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        if fields.next()? != device {
            return None;
        }
        Some(PathBuf::from(unescape(fields.next()?)))
    })
}

/// `/proc/mounts` writes spaces and other awkward bytes as `\040`-style octal escapes.
fn unescape(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let octal = (bytes[i] == b'\\' && i + 4 <= bytes.len())
            .then(|| std::str::from_utf8(&bytes[i + 1..i + 4]).ok().and_then(|d| u8::from_str_radix(d, 8).ok()))
            .flatten();
        match octal {
            Some(byte) => {
                out.push(byte);
                i += 4;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn trimmed(path: String) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

pub(crate) fn candidates() -> Result<Vec<Candidate>> {
    let Ok(entries) = std::fs::read_dir("/sys/class/block") else {
        return Ok(Vec::new());
    };
    let mut found: Vec<Candidate> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let number = name.strip_prefix("sr")?;
            if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let vendor = trimmed(format!("/sys/block/{name}/device/vendor"));
            let model = trimmed(format!("/sys/block/{name}/device/model"));
            let label = [vendor, model].into_iter().flatten().collect::<Vec<_>>().join(" ");
            Some(Candidate {
                id: DriveId(format!("/dev/{name}")),
                name: if label.is_empty() { "Disc drive".to_owned() } else { label },
            })
        })
        .collect();
    found.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(found)
}

pub(crate) fn eject(id: &DriveId) -> Result<()> {
    let fd = open_fd(id)?;
    // SAFETY: `fd` is an open descriptor; CDROMEJECT takes no argument.
    let result = unsafe { libc::ioctl(fd.as_raw_fd(), CDROMEJECT as _) };
    if result == -1 {
        Err(DiscError::Busy)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn sg_io_hdr_has_the_kernel_size() {
        assert_eq!(std::mem::size_of::<SgIoHdr>(), 88);
    }

    #[test]
    fn mount_table_is_parsed_with_escapes() {
        let table = "proc /proc proc rw 0 0\n/dev/sr0 /run/media/ana/My\\040Album udf ro 0 0\n";
        assert_eq!(parse_mounts(table, "/dev/sr0"), Some(PathBuf::from("/run/media/ana/My Album")));
        assert_eq!(parse_mounts(table, "/dev/sr1"), None);
    }

    #[test]
    fn only_sr_devices_are_accepted() {
        assert!(device_path(&DriveId("/dev/sr0".into())).is_ok());
        assert!(device_path(&DriveId("/dev/sda".into())).is_err());
        assert!(device_path(&DriveId("/dev/sr0/../sda".into())).is_err());
    }
}
