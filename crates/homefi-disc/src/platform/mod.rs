//! OS transports behind one small interface: which optical drives exist, how to open one as a
//! [`crate::drives::Device`], how to eject. Everything above works on [`crate::SectorSource`].
//!
//! * macOS: `DKIOCCD*` ioctls on `/dev/rdiskN` (`macos.rs`).
//! * Linux: `SG_IO` on `/dev/srN` (`linux.rs`, MMC commands from `scsi.rs`).
//! * Windows: SPTI on `\\.\D:` (`windows.rs`, MMC commands from `scsi.rs`).

use crate::drives::DriveId;

/// A drive found by the OS, before its disc is looked at.
pub(crate) struct Candidate {
    pub id: DriveId,
    pub name: String,
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(crate) use linux::{candidates, eject, open};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::{candidates, eject, open};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::{candidates, eject, open};

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
mod unsupported {
    use super::Candidate;
    use crate::drives::{Device, DriveId};
    use crate::error::{DiscError, Result};

    pub(crate) fn candidates() -> Result<Vec<Candidate>> {
        Ok(Vec::new())
    }
    pub(crate) fn open(_: &DriveId) -> Result<Box<dyn Device>> {
        Err(DiscError::NoDrive)
    }
    pub(crate) fn eject(_: &DriveId) -> Result<()> {
        Err(DiscError::NoDrive)
    }
}
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub(crate) use unsupported::{candidates, eject, open};
