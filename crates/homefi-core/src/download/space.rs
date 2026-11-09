//! Free-space rule for downloads: one place that says whether a folder still has room.
//!
//! Desktop and Android both ask [`check`] before a job, so a nearly full disk (a TV box's internal
//! storage, a USB stick) is noticed before a half-written album is, and the app can offer another place.

use std::io;
use std::path::Path;

/// Free space a download target must keep, in megabytes, unless the user changes it.
pub const DEFAULT_MIN_FREE_MB: u32 = 500;
/// Lowest and highest value the setting accepts.
pub const MIN_FREE_MB_RANGE: std::ops::RangeInclusive<u32> = 100..=5000;

const BYTES_PER_MB: u64 = 1024 * 1024;

/// What [`check`] measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FreeSpace {
    /// Bytes an unprivileged process can still write.
    pub free_bytes: u64,
    /// The bytes that had to stay free.
    pub min_free_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SpaceError {
    #[error("not enough free space: {free_bytes} bytes free, {min_free_bytes} needed")]
    NotEnough { free_bytes: u64, min_free_bytes: u64 },
    /// The volume could not be measured (gone, unsupported platform). Callers should not block a
    /// download only because of this: the write itself reports a real failure.
    #[error("could not read the free space: {0}")]
    Unreadable(String),
}

impl From<SpaceError> for crate::Error {
    /// Shown as "the disk is full" (`Problem::DiskFull`) in the user's language.
    fn from(e: SpaceError) -> Self {
        crate::Error::Io(io::Error::new(io::ErrorKind::StorageFull, e))
    }
}

/// A saved setting brought into [`MIN_FREE_MB_RANGE`]; `0` stays `0` (the check is off).
pub fn clamp_min_free_mb(mb: u32) -> u32 {
    if mb == 0 {
        0
    } else {
        mb.clamp(*MIN_FREE_MB_RANGE.start(), *MIN_FREE_MB_RANGE.end())
    }
}

/// Free bytes on the volume holding `dir` (which may not exist yet: its nearest existing parent is measured).
pub fn free_bytes(dir: &Path) -> io::Result<u64> {
    let existing = dir.ancestors().find(|p| p.exists()).ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
    available_bytes(existing)
}

#[cfg(unix)]
fn available_bytes(path: &Path) -> io::Result<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c_path =
        CString::new(path.as_os_str().as_bytes()).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::zeroed();
    // SAFETY: `c_path` is a valid NUL-terminated string and `stat` points to writable memory of the right type.
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `statvfs` returned 0, so it filled the struct.
    let stat = unsafe { stat.assume_init() };
    // the field types differ per platform (u32 / u64)
    #[allow(clippy::unnecessary_cast)]
    Ok((stat.f_bavail as u64).saturating_mul(stat.f_frsize as u64))
}

#[cfg(not(unix))]
fn available_bytes(_path: &Path) -> io::Result<u64> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "free space is not read on this platform"))
}

/// The rule, separated from the system call so tests can fake the measurement.
fn judge(free: io::Result<u64>, min_free_mb: u32) -> Result<FreeSpace, SpaceError> {
    let min_free_bytes = u64::from(clamp_min_free_mb(min_free_mb)) * BYTES_PER_MB;
    let free_bytes = free.map_err(|e| SpaceError::Unreadable(e.to_string()))?;
    if free_bytes < min_free_bytes {
        Err(SpaceError::NotEnough { free_bytes, min_free_bytes })
    } else {
        Ok(FreeSpace { free_bytes, min_free_bytes })
    }
}

/// Does the volume holding `dir` keep at least `min_free_mb` megabytes free? (`0` disables the rule.)
pub fn check(dir: &Path, min_free_mb: u32) -> Result<FreeSpace, SpaceError> {
    judge(free_bytes(dir), min_free_mb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enough_room_passes() {
        let ok = judge(Ok(800 * BYTES_PER_MB), 500).unwrap();
        assert_eq!(ok, FreeSpace { free_bytes: 800 * BYTES_PER_MB, min_free_bytes: 500 * BYTES_PER_MB });
    }

    #[test]
    fn exactly_the_minimum_passes_one_byte_less_fails() {
        assert!(judge(Ok(500 * BYTES_PER_MB), 500).is_ok());
        let err = judge(Ok(500 * BYTES_PER_MB - 1), 500).unwrap_err();
        assert_eq!(
            err,
            SpaceError::NotEnough { free_bytes: 500 * BYTES_PER_MB - 1, min_free_bytes: 500 * BYTES_PER_MB }
        );
    }

    #[test]
    fn an_unreadable_volume_is_its_own_error() {
        let err = judge(Err(io::Error::from(io::ErrorKind::NotFound)), 500).unwrap_err();
        assert!(matches!(err, SpaceError::Unreadable(_)));
    }

    #[test]
    fn the_setting_is_clamped_and_zero_turns_the_rule_off() {
        assert_eq!(clamp_min_free_mb(5), 100);
        assert_eq!(clamp_min_free_mb(999_999), 5000);
        assert_eq!(clamp_min_free_mb(500), 500);
        assert_eq!(clamp_min_free_mb(0), 0);
        assert!(judge(Ok(0), 0).is_ok());
    }

    #[test]
    fn a_missing_folder_is_measured_through_its_parent() {
        let dir = tempfile::tempdir().unwrap();
        let later = dir.path().join("Music").join("HomeFi");
        // a temp dir always has some room; asking for the minimum 100 MB can only fail on a nearly full CI disk
        assert!(free_bytes(&later).is_ok() || cfg!(not(unix)));
    }

    #[test]
    fn running_out_of_space_reads_as_a_full_disk() {
        let e: crate::Error = SpaceError::NotEnough { free_bytes: 1, min_free_bytes: 2 }.into();
        assert_eq!(e.problem(), crate::humane::Problem::DiskFull);
    }
}
