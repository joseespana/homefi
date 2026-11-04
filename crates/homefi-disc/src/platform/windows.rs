//! Windows transport: SPTI (`IOCTL_SCSI_PASS_THROUGH_DIRECT`) on `\\.\D:` carrying the MMC
//! commands of `scsi.rs`. Optical devices can be opened for pass-through without elevation
//! (the same way other rippers run unelevated); a refusal is reported as a permission problem.

use std::ffi::c_void;
use std::mem::{offset_of, size_of};
use std::path::PathBuf;

use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, GetDriveTypeW, GetLogicalDrives, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::Storage::IscsiDisc::{
    IOCTL_SCSI_PASS_THROUGH_DIRECT, SCSI_IOCTL_DATA_IN, SCSI_IOCTL_DATA_UNSPECIFIED, SCSI_PASS_THROUGH_DIRECT,
};
use windows_sys::Win32::System::Ioctl::IOCTL_STORAGE_EJECT_MEDIA;
use windows_sys::Win32::System::IO::DeviceIoControl;

use super::Candidate;
use crate::drives::{Device, DriveId};
use crate::error::{DiscError, Result};
use crate::scsi::{check_condition, ScsiDevice, ScsiError, ScsiTransport};

/// `GetDriveTypeW` result for optical drives (`DRIVE_CDROM`).
const DRIVE_CDROM: u32 = 5;
const SCSI_STATUS_CHECK_CONDITION: u8 = 0x02;
const SENSE_BYTES: usize = 32;
/// Seconds SPTI waits for a command; reads may wait for a drive retrying a bad sector.
const COMMAND_TIMEOUT_S: u32 = 60;
/// Largest transfer we make (27 sectors with C2 is 71 KB).
const MAX_TRANSFER: usize = 128 * 1024;
/// Adapters may demand aligned buffers; 512 satisfies every one.
const BUFFER_ALIGN: usize = 512;
const ERROR_ACCESS_DENIED: i32 = 5;
const ERROR_SHARING_VIOLATION: i32 = 32;

/// The structure SPTI documents for commands with sense data.
#[repr(C)]
struct PassThrough {
    header: SCSI_PASS_THROUGH_DIRECT,
    filler: u32,
    sense: [u8; SENSE_BYTES],
}

struct Handle(HANDLE);

// SAFETY: a Windows file handle is just a reference to a kernel object; it may be used from
// any thread. `Handle` is used by one thread at a time (the reader that owns the drive).
unsafe impl Send for Handle {}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle was returned by `CreateFileW` and is closed exactly once, here.
        unsafe { CloseHandle(self.0) };
    }
}

struct Spti {
    handle: Handle,
    /// Over-allocated so an aligned window of `MAX_TRANSFER` bytes always fits.
    storage: Vec<u8>,
}

impl Spti {
    fn aligned_offset(&self) -> usize {
        (BUFFER_ALIGN - self.storage.as_ptr() as usize % BUFFER_ALIGN) % BUFFER_ALIGN
    }
}

impl ScsiTransport for Spti {
    fn execute(&mut self, cdb: &[u8], data: &mut [u8]) -> std::result::Result<usize, ScsiError> {
        if cdb.len() > 16 || data.len() > MAX_TRANSFER {
            return Err(ScsiError::Os(std::io::Error::from(std::io::ErrorKind::InvalidInput)));
        }
        let start = self.aligned_offset();
        let buffer = &mut self.storage[start..start + MAX_TRANSFER];
        // SAFETY: `SCSI_PASS_THROUGH_DIRECT` is plain data for which all-zero is a valid value.
        let mut request: PassThrough = unsafe { std::mem::zeroed() };
        request.header.Length = size_of::<SCSI_PASS_THROUGH_DIRECT>() as u16;
        request.header.CdbLength = cdb.len() as u8;
        request.header.SenseInfoLength = SENSE_BYTES as u8;
        request.header.DataIn =
            if data.is_empty() { SCSI_IOCTL_DATA_UNSPECIFIED as u8 } else { SCSI_IOCTL_DATA_IN as u8 };
        request.header.DataTransferLength = data.len() as u32;
        request.header.TimeOutValue = COMMAND_TIMEOUT_S;
        request.header.DataBuffer =
            if data.is_empty() { std::ptr::null_mut() } else { buffer.as_mut_ptr().cast::<c_void>() };
        request.header.SenseInfoOffset = offset_of!(PassThrough, sense) as u32;
        request.header.Cdb[..cdb.len()].copy_from_slice(cdb);
        let mut returned = 0u32;
        let size = size_of::<PassThrough>() as u32;
        // SAFETY: the handle is open for the life of `self`; `request` is a `repr(C)` mirror of
        // the structure SPTI documents, and its `DataBuffer` points into `buffer`, which is
        // aligned, at least `data.len()` bytes long and untouched until the call returns.
        let ok = unsafe {
            DeviceIoControl(
                self.handle.0,
                IOCTL_SCSI_PASS_THROUGH_DIRECT,
                (&raw const request).cast(),
                size,
                (&raw mut request).cast(),
                size,
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(ScsiError::Os(std::io::Error::last_os_error()));
        }
        if request.header.ScsiStatus == SCSI_STATUS_CHECK_CONDITION {
            return Err(check_condition(&request.sense));
        }
        if request.header.ScsiStatus != 0 {
            return Err(ScsiError::Os(std::io::Error::other(format!("SCSI status {:#x}", request.header.ScsiStatus))));
        }
        let got = (request.header.DataTransferLength as usize).min(data.len());
        data[..got].copy_from_slice(&buffer[..got]);
        Ok(got)
    }
}

/// `D:` -> the letter, for ids made of one ASCII letter and a colon.
fn letter(id: &DriveId) -> Result<char> {
    let mut chars = id.0.chars();
    match (chars.next(), chars.next(), chars.next()) {
        (Some(c), Some(':'), None) if c.is_ascii_alphabetic() => Ok(c.to_ascii_uppercase()),
        _ => Err(DiscError::NoDrive),
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn open_handle(id: &DriveId) -> Result<Handle> {
    let path = wide(&format!(r"\\.\{}:", letter(id)?));
    // SAFETY: `path` is a NUL-terminated UTF-16 string that outlives the call; no security
    // attributes or template are used.
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        let error = std::io::Error::last_os_error();
        return Err(match error.raw_os_error() {
            Some(ERROR_ACCESS_DENIED) => DiscError::Permission {
                hint: "Try running HomeFi as the same user that is signed in to Windows, and close other programs using the disc."
                    .into(),
            },
            Some(ERROR_SHARING_VIOLATION) => DiscError::Busy,
            _ => DiscError::Io(error),
        });
    }
    Ok(Handle(handle))
}

pub(crate) fn open(id: &DriveId) -> Result<Box<dyn Device>> {
    let handle = open_handle(id)?;
    let root = PathBuf::from(format!("{}:\\", letter(id)?));
    let transport = Spti { handle, storage: vec![0; MAX_TRANSFER + BUFFER_ALIGN] };
    // Windows shows an audio CD's tracks as .cda files, which is fine: audio is detected first.
    Ok(Box::new(ScsiDevice::new(transport, Box::new(move || root.read_dir().is_ok().then(|| root.clone())))))
}

pub(crate) fn candidates() -> Result<Vec<Candidate>> {
    // SAFETY: plain Win32 call without arguments.
    let mask = unsafe { GetLogicalDrives() };
    Ok((0..26u8)
        .filter(|bit| mask & (1 << bit) != 0)
        .filter_map(|bit| {
            let letter = char::from(b'A' + bit);
            let root = wide(&format!("{letter}:\\"));
            // SAFETY: `root` is a NUL-terminated UTF-16 string that outlives the call.
            let kind = unsafe { GetDriveTypeW(root.as_ptr()) };
            (kind == DRIVE_CDROM)
                .then(|| Candidate { id: DriveId(format!("{letter}:")), name: format!("Disc drive ({letter}:)") })
        })
        .collect())
}

pub(crate) fn eject(id: &DriveId) -> Result<()> {
    let handle = open_handle(id)?;
    let mut returned = 0u32;
    // SAFETY: the handle is open; this control code takes no input or output buffers.
    let ok = unsafe {
        DeviceIoControl(
            handle.0,
            IOCTL_STORAGE_EJECT_MEDIA,
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            0,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        Err(DiscError::Busy)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_drive_letters_are_accepted() {
        assert_eq!(letter(&DriveId("d:".into())).unwrap(), 'D');
        assert!(letter(&DriveId("D:\\".into())).is_err());
        assert!(letter(&DriveId("..".into())).is_err());
        assert!(letter(&DriveId("1:".into())).is_err());
    }

    #[test]
    fn pass_through_has_the_documented_sense_offset() {
        assert_eq!(offset_of!(PassThrough, sense) % 4, 0);
    }
}
