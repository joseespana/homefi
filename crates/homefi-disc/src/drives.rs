//! Drives and what is in them: listing, opening, ejecting, and telling an audio CD from a
//! data disc or a copy-protected DVD / Blu-ray.
//!
//! Protection is only *detected* (so the screen can say why a disc can't be copied). Nothing
//! here reads encrypted content, asks for keys or decrypts.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cdtext::CdText;
use crate::error::{DiscError, Result};
use crate::platform;
use crate::source::{Capabilities, ReadFlags, ReadSpeed, SectorSource};
use crate::toc::Toc;

/// Opaque to the UI: `disk4`, `/dev/sr0` or `D:` depending on the OS.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DriveId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProtectionKind {
    /// DVD-Video (Content Scramble System).
    DvdCss,
    /// DVD-Audio (Content Protection for Prerecorded Media) and recordable-media variants.
    DvdCppm,
    /// Blu-ray (AACS).
    BluRayAacs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Media {
    None,
    AudioCd {
        tracks: u8,
        length_ms: u64,
    },
    /// A disc with files (CD-ROM, DVD-ROM, home recordings). `mount` is where the OS shows it.
    DataDisc {
        mount: Option<PathBuf>,
    },
    Protected(ProtectionKind),
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveInfo {
    pub id: DriveId,
    /// Vendor and model when the OS tells.
    pub name: String,
    pub media: Media,
}

/// What the OS-specific code can see of an open drive, beyond reading audio.
pub(crate) trait Device: SectorSource {
    /// Finds out what the drive can do (C2, CD-Text): a few reads, so only when a session opens.
    fn prepare(&mut self) {}
    /// Copyright protection system type of a DVD (READ DISC STRUCTURE format 1): `Some(0)` =
    /// none. `None` when the disc is not a DVD or the drive can't say.
    fn dvd_copyright(&mut self) -> Option<u8>;
    /// Where the OS mounted the disc, if it did.
    fn mount_point(&self) -> Option<PathBuf>;
}

/// An open drive. `Send`, not `Sync`: one reader owns it.
pub struct Drive {
    id: DriveId,
    device: Box<dyn Device>,
}

impl Drive {
    pub(crate) fn new(id: DriveId, device: Box<dyn Device>) -> Self {
        Drive { id, device }
    }

    pub fn id(&self) -> &DriveId {
        &self.id
    }

    /// What is in the drive right now.
    pub fn media(&mut self) -> Media {
        classify(&probe(self.device.as_mut()))
    }

    /// Gives the drive to a reader (for [`crate::read_track`] or the rip pipeline).
    pub fn into_source(self) -> Box<dyn SectorSource> {
        Box::new(self)
    }
}

impl SectorSource for Drive {
    fn toc(&mut self) -> Result<Toc> {
        self.device.toc()
    }
    fn read_audio(&mut self, lba: u32, count: u16, pcm: &mut [u8], c2: Option<&mut [u8]>) -> Result<ReadFlags> {
        self.device.read_audio(lba, count, pcm, c2)
    }
    fn cd_text(&mut self) -> Result<Option<CdText>> {
        self.device.cd_text()
    }
    fn isrc(&mut self, track: u8) -> Result<Option<String>> {
        self.device.isrc(track)
    }
    fn mcn(&mut self) -> Result<Option<String>> {
        self.device.mcn()
    }
    fn set_speed(&mut self, speed: ReadSpeed) -> Result<()> {
        self.device.set_speed(speed)
    }
    fn capabilities(&self) -> Capabilities {
        self.device.capabilities()
    }
}

/// Drives with a disc in them or able to take one, with what is in each. Cheap: one TOC read
/// per drive, no spin-up for drives without media.
pub fn list_drives() -> Result<Vec<DriveInfo>> {
    let mut drives = Vec::new();
    for candidate in platform::candidates()? {
        let media = match platform::open(&candidate.id) {
            Ok(mut device) => classify(&probe(device.as_mut())),
            Err(DiscError::NoDisc) => Media::None,
            Err(error) => {
                tracing::debug!(drive = %candidate.id.0, %error, "drive could not be opened for listing");
                Media::Unknown
            }
        };
        drives.push(DriveInfo { id: candidate.id, name: candidate.name, media });
    }
    Ok(drives)
}

pub fn open(id: &DriveId) -> Result<Drive> {
    let mut device = platform::open(id)?;
    device.prepare();
    Ok(Drive::new(id.clone(), device))
}

pub fn eject(id: &DriveId) -> Result<()> {
    platform::eject(id)
}

// ---- media detection ----------------------------------------------------------------------

/// Facts gathered from a drive; [`classify`] turns them into a [`Media`].
#[derive(Debug, Default)]
pub(crate) struct Probe {
    pub present: bool,
    pub toc: Option<Toc>,
    pub dvd_copyright: Option<u8>,
    pub mount: Option<PathBuf>,
}

pub(crate) fn probe(device: &mut dyn Device) -> Probe {
    let mut probe = Probe { present: true, ..Probe::default() };
    match device.toc() {
        Ok(toc) => probe.toc = Some(toc),
        Err(DiscError::NoDisc) => {
            probe.present = false;
            return probe;
        }
        Err(_) => {}
    }
    let has_audio = probe.toc.as_ref().is_some_and(|toc| toc.audio_tracks().next().is_some());
    if !has_audio {
        probe.dvd_copyright = device.dvd_copyright();
        probe.mount = device.mount_point();
    }
    probe
}

pub(crate) fn classify(probe: &Probe) -> Media {
    if !probe.present {
        return Media::None;
    }
    if let Some(toc) = &probe.toc {
        let tracks = toc.audio_tracks().count();
        if tracks > 0 {
            return Media::AudioCd { tracks: tracks.min(usize::from(u8::MAX)) as u8, length_ms: toc.length_ms() };
        }
    }
    let mount = probe.mount.as_deref();
    if probe.dvd_copyright.is_some_and(|system| system != 0) {
        let audio_dvd = mount.is_some_and(is_dvd_audio);
        return Media::Protected(if audio_dvd { ProtectionKind::DvdCppm } else { ProtectionKind::DvdCss });
    }
    if mount.is_some_and(|dir| dir.join("AACS").is_dir()) {
        return Media::Protected(ProtectionKind::BluRayAacs);
    }
    if probe.mount.is_some() || probe.toc.is_some() || probe.dvd_copyright == Some(0) {
        return Media::DataDisc { mount: probe.mount.clone() };
    }
    Media::Unknown
}

/// DVD-Audio discs carry `AUDIO_TS/*.AOB` and an empty `VIDEO_TS`.
fn is_dvd_audio(mount: &Path) -> bool {
    let has_aob = std::fs::read_dir(mount.join("AUDIO_TS")).is_ok_and(|entries| {
        entries.flatten().any(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("aob")))
    });
    let has_video = std::fs::read_dir(mount.join("VIDEO_TS")).is_ok_and(|entries| {
        entries.flatten().any(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("vob")))
    });
    has_aob && !has_video
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toc::RawTocEntry;

    fn audio_toc() -> Toc {
        Toc::from_entries(&[RawTocEntry { number: 1, control: 0, start_lba: 0 }], 75 * 120)
    }

    fn data_toc() -> Toc {
        Toc::from_entries(&[RawTocEntry { number: 1, control: 4, start_lba: 0 }], 75 * 120)
    }

    #[test]
    fn audio_cd_is_recognised() {
        let probe = Probe { present: true, toc: Some(audio_toc()), ..Probe::default() };
        assert_eq!(classify(&probe), Media::AudioCd { tracks: 1, length_ms: 120_000 });
    }

    #[test]
    fn empty_drive_is_none() {
        assert_eq!(classify(&Probe::default()), Media::None);
    }

    #[test]
    fn data_cd_is_a_data_disc_with_its_mount() {
        let probe =
            Probe { present: true, toc: Some(data_toc()), mount: Some("/Volumes/X".into()), ..Probe::default() };
        assert_eq!(classify(&probe), Media::DataDisc { mount: Some("/Volumes/X".into()) });
    }

    #[test]
    fn protected_dvd_video_is_css() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("VIDEO_TS")).unwrap();
        std::fs::write(dir.path().join("VIDEO_TS/VTS_01_1.VOB"), b"x").unwrap();
        let probe = Probe { present: true, dvd_copyright: Some(1), mount: Some(dir.path().into()), ..Probe::default() };
        assert_eq!(classify(&probe), Media::Protected(ProtectionKind::DvdCss));
    }

    #[test]
    fn protected_dvd_audio_is_cppm() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("AUDIO_TS")).unwrap();
        std::fs::write(dir.path().join("AUDIO_TS/ATS_01_1.AOB"), b"x").unwrap();
        std::fs::create_dir(dir.path().join("VIDEO_TS")).unwrap();
        let probe = Probe { present: true, dvd_copyright: Some(1), mount: Some(dir.path().into()), ..Probe::default() };
        assert_eq!(classify(&probe), Media::Protected(ProtectionKind::DvdCppm));
    }

    #[test]
    fn home_dvd_without_protection_is_data() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("VIDEO_TS")).unwrap();
        let probe = Probe { present: true, dvd_copyright: Some(0), mount: Some(dir.path().into()), ..Probe::default() };
        assert!(matches!(classify(&probe), Media::DataDisc { .. }));
    }

    #[test]
    fn blu_ray_with_aacs_folder_is_protected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("AACS")).unwrap();
        std::fs::create_dir(dir.path().join("BDMV")).unwrap();
        let probe = Probe { present: true, mount: Some(dir.path().into()), ..Probe::default() };
        assert_eq!(classify(&probe), Media::Protected(ProtectionKind::BluRayAacs));
    }

    #[test]
    fn a_disc_nobody_can_describe_is_unknown() {
        assert_eq!(classify(&Probe { present: true, ..Probe::default() }), Media::Unknown);
    }

    /// A drive made of an image, to run `probe` end to end.
    struct Fake(crate::ImageSource, Option<u8>);
    impl SectorSource for Fake {
        fn toc(&mut self) -> Result<Toc> {
            self.0.toc()
        }
        fn read_audio(&mut self, l: u32, c: u16, p: &mut [u8], c2: Option<&mut [u8]>) -> Result<ReadFlags> {
            self.0.read_audio(l, c, p, c2)
        }
        fn cd_text(&mut self) -> Result<Option<CdText>> {
            Ok(None)
        }
        fn isrc(&mut self, _: u8) -> Result<Option<String>> {
            Ok(None)
        }
        fn mcn(&mut self) -> Result<Option<String>> {
            Ok(None)
        }
        fn set_speed(&mut self, _: ReadSpeed) -> Result<()> {
            Ok(())
        }
        fn capabilities(&self) -> Capabilities {
            Capabilities::default()
        }
    }
    impl Device for Fake {
        fn dvd_copyright(&mut self) -> Option<u8> {
            self.1
        }
        fn mount_point(&self) -> Option<PathBuf> {
            None
        }
    }

    #[test]
    fn drive_reports_its_media() {
        let mut drive =
            Drive::new(DriveId("fake".into()), Box::new(Fake(crate::ImageSource::synthetic(&[750, 750]), None)));
        assert_eq!(drive.media(), Media::AudioCd { tracks: 2, length_ms: 20_000 });
        assert_eq!(drive.id().0, "fake");
    }
}
