//! HomeFi desktop discs: drives, table of contents and ids, secure audio-CD reading, and
//! media / copy-protection detection. Desktop only (no Android target).
//!
//! Layers: [`SectorSource`] is the only thing that touches hardware (per-OS transports in
//! `platform`, MMC commands in `scsi`); [`read_track`] is the secure reader written over it;
//! everything is tested with [`ImageSource`] and [`FlakySource`], no drive involved.
//!
//! HomeFi never circumvents copy protection: CSS, CPPM and AACS are only *detected*
//! ([`Media::Protected`]) so the screen can say why a disc can't be copied.

pub mod cdtext;
pub mod checksum;
pub mod chunks;
pub mod cue;
pub mod drives;
pub mod encode;
pub mod error;
pub mod identify;
pub mod image;
pub mod log;
mod platform;
pub mod read;
pub mod rip;
mod scsi;
pub mod settings;
pub mod source;
pub mod toc;

pub use cdtext::{parse_cd_text, CdText, CdTextTrack};
pub use chunks::{chunk_channel, Chunk, ChunkConsumer, ChunkProducer, DEFAULT_CHUNKS};
pub use drives::{eject, list_drives, open, Drive, DriveId, DriveInfo, Media, ProtectionKind};
pub use encode::{Mp3Kind, RipFormat, Verified};
pub use error::{DiscError, Result};
pub use identify::{
    fetch_cover, identify, AlbumMeta, CoverRef, DiscFacts, DiscMatch, MusicBrainz, MusicBrainzEndpoints, Source,
    TrackMeta,
};
pub use image::{FlakySource, ImageSource};
pub use read::{read_track, C2Use, ReadMode, ReadPlan, ReadProgress, TrackRead};
pub use rip::{rip, RipOptions, RipOutput, RipProgress, RipRequest, RipSummary};
pub use settings::{lenient_disc, DiscSettings, OnUnreadable};
pub use source::{Capabilities, ReadFlags, ReadSpeed, SectorSource};
pub use toc::{RawTocEntry, Toc, TocTrack};
