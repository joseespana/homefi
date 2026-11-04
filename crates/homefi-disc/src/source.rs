//! The seam between hardware and rules: everything above [`SectorSource`] is tested with
//! disc images, no drive involved.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::cdtext::CdText;
use crate::error::Result;
use crate::toc::Toc;

/// Bytes of audio in one CD-DA sector (588 stereo frames of 16-bit samples).
pub const SECTOR_BYTES: usize = 2352;
/// Bytes of C2 error bits per sector: one bit for each byte of audio.
pub const C2_BYTES: usize = 294;
/// Stereo frames (one sample per channel) in a sector.
pub const FRAMES_PER_SECTOR: usize = 588;
/// `i16` values (both channels) in a sector.
pub const SAMPLES_PER_SECTOR: usize = FRAMES_PER_SECTOR * 2;
pub const SECTORS_PER_SECOND: u32 = 75;
/// The two-second pre-gap that TOC addresses in MSF form include.
pub const PREGAP_SECTORS: u32 = 150;
/// Sectors between the audio and the data session of a CD-Extra (lead-out + lead-in + pre-gap).
pub const CD_EXTRA_GAP_SECTORS: u32 = 11_400;
/// Sectors per read window and per pipeline chunk (27 x 2352 = 63 504 bytes).
pub const CHUNK_SECTORS: u16 = 27;
/// 1x speed of a CD in kB/s (what SET CD SPEED expects).
pub const KBPS_PER_X: u16 = 176;

/// What a successful read filled besides the audio.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReadFlags {
    /// The C2 buffer holds real error bits (the drive reported them).
    pub c2: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Capabilities {
    /// The drive can return C2 error pointers with the audio.
    pub c2: bool,
    /// The drive delivers a continuous, sample-accurate stream (no jitter between reads).
    pub accurate_stream: bool,
    /// The drive can report CD-Text.
    pub cd_text: bool,
}

/// Requested read speed; best effort, drives may ignore it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReadSpeed {
    Max,
    /// Let the drive choose (typical for secure reads on modern drives).
    #[default]
    Auto,
    /// A multiple of 1x (1..=52).
    X(u8),
}

impl ReadSpeed {
    pub const MAX_X: u8 = 52;

    /// kB/s for SET CD SPEED: `None` = leave the drive alone, `0xFFFF` = maximum.
    pub fn kbps(self) -> Option<u16> {
        match self {
            ReadSpeed::Auto => None,
            ReadSpeed::Max => Some(0xFFFF),
            ReadSpeed::X(x) => Some(u16::from(x.clamp(1, Self::MAX_X)) * KBPS_PER_X),
        }
    }
}

impl Serialize for ReadSpeed {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            ReadSpeed::Max => serializer.serialize_str("max"),
            ReadSpeed::Auto => serializer.serialize_str("auto"),
            ReadSpeed::X(x) => serializer.serialize_u8(*x),
        }
    }
}

impl<'de> Deserialize<'de> for ReadSpeed {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Number(u64),
            Text(String),
        }
        Ok(match Repr::deserialize(deserializer)? {
            Repr::Number(x) => ReadSpeed::X(x.clamp(1, u64::from(ReadSpeed::MAX_X)) as u8),
            Repr::Text(text) => match text.trim().to_lowercase().as_str() {
                "max" => ReadSpeed::Max,
                other => other
                    .trim_end_matches('x')
                    .parse::<u64>()
                    .map_or(ReadSpeed::Auto, |x| ReadSpeed::X(x.clamp(1, u64::from(ReadSpeed::MAX_X)) as u8)),
            },
        })
    }
}

/// The only thing that touches a drive. Implementations never allocate in `read_audio`.
pub trait SectorSource: Send {
    fn toc(&mut self) -> Result<Toc>;
    /// Reads `count` audio sectors from `lba`; fills `pcm` (count x 2352 bytes) and, when the
    /// drive supports it, `c2` (count x 294 bytes of error bits). Returns which it filled.
    fn read_audio(&mut self, lba: u32, count: u16, pcm: &mut [u8], c2: Option<&mut [u8]>) -> Result<ReadFlags>;
    /// `None` when the disc carries no CD-Text.
    fn cd_text(&mut self) -> Result<Option<CdText>>;
    /// Slow (about 0.5 to 1 s per track): call from the identify step, cancellable.
    fn isrc(&mut self, track: u8) -> Result<Option<String>>;
    fn mcn(&mut self) -> Result<Option<String>>;
    /// Best effort; failures are not worth stopping for.
    fn set_speed(&mut self, speed: ReadSpeed) -> Result<()>;
    fn capabilities(&self) -> Capabilities;
}

impl<T: SectorSource + ?Sized> SectorSource for Box<T> {
    fn toc(&mut self) -> Result<Toc> {
        (**self).toc()
    }
    fn read_audio(&mut self, lba: u32, count: u16, pcm: &mut [u8], c2: Option<&mut [u8]>) -> Result<ReadFlags> {
        (**self).read_audio(lba, count, pcm, c2)
    }
    fn cd_text(&mut self) -> Result<Option<CdText>> {
        (**self).cd_text()
    }
    fn isrc(&mut self, track: u8) -> Result<Option<String>> {
        (**self).isrc(track)
    }
    fn mcn(&mut self) -> Result<Option<String>> {
        (**self).mcn()
    }
    fn set_speed(&mut self, speed: ReadSpeed) -> Result<()> {
        (**self).set_speed(speed)
    }
    fn capabilities(&self) -> Capabilities {
        (**self).capabilities()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_round_trips_as_text_or_number() {
        for (speed, json) in [(ReadSpeed::Max, "\"max\""), (ReadSpeed::Auto, "\"auto\""), (ReadSpeed::X(8), "8")] {
            assert_eq!(serde_json::to_string(&speed).unwrap(), json);
            assert_eq!(serde_json::from_str::<ReadSpeed>(json).unwrap(), speed);
        }
        assert_eq!(serde_json::from_str::<ReadSpeed>("99").unwrap(), ReadSpeed::X(52));
        assert_eq!(serde_json::from_str::<ReadSpeed>("\"junk\"").unwrap(), ReadSpeed::Auto);
    }

    #[test]
    fn speed_in_kilobytes() {
        assert_eq!(ReadSpeed::X(4).kbps(), Some(704));
        assert_eq!(ReadSpeed::Auto.kbps(), None);
        assert_eq!(ReadSpeed::Max.kbps(), Some(0xFFFF));
    }
}
