//! The table of contents and every id computed from it. Pure: no drive needed.
//!
//! LBAs are 0-based (the audio of track 1 usually starts at LBA 0); the "+150"
//! pre-gap shows up only where a published algorithm asks for it.

use std::fmt::Write as _;

use base64::Engine as _;
use serde::Serialize;
use sha1::{Digest, Sha1};

use crate::source::{CD_EXTRA_GAP_SECTORS, FRAMES_PER_SECTOR, PREGAP_SECTORS, SECTORS_PER_SECOND};

/// MusicBrainz and CDDB lay out 99 track slots.
const MAX_TRACKS: u8 = 99;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TocTrack {
    pub number: u8,
    pub start_lba: u32,
    /// Exclusive. For the audio track before a data session (CD-Extra) this already
    /// leaves out the 11 400-sector session gap.
    pub end_lba: u32,
    pub audio: bool,
    pub pre_emphasis: bool,
}

impl TocTrack {
    pub fn sectors(&self) -> u32 {
        self.end_lba.saturating_sub(self.start_lba)
    }
}

/// One TOC line as a drive reports it, before gaps are worked out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawTocEntry {
    pub number: u8,
    /// Q-channel control nibble: bit 2 = data track, bit 0 = pre-emphasis (audio).
    pub control: u8,
    pub start_lba: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Toc {
    pub first: u8,
    pub last: u8,
    pub leadout_lba: u32,
    pub tracks: Vec<TocTrack>,
}

impl Toc {
    /// Builds the TOC from the entries a drive returns (any order) and the lead-out.
    /// Entries are sorted by number; each track ends where the next starts, except an
    /// audio track followed by the last (data) track of a CD-Extra, which ends
    /// [`CD_EXTRA_GAP_SECTORS`] earlier.
    pub fn from_entries(entries: &[RawTocEntry], leadout_lba: u32) -> Toc {
        let mut sorted = entries.to_vec();
        sorted.sort_by_key(|entry| entry.number);
        let mut tracks = Vec::with_capacity(sorted.len());
        for (i, entry) in sorted.iter().enumerate() {
            let audio = entry.control & 0x04 == 0;
            let next = sorted.get(i + 1);
            let end_lba = match next {
                Some(next) if audio && next.control & 0x04 != 0 && i + 2 == sorted.len() => {
                    next.start_lba.saturating_sub(CD_EXTRA_GAP_SECTORS).max(entry.start_lba)
                }
                Some(next) => next.start_lba,
                None => leadout_lba,
            };
            tracks.push(TocTrack {
                number: entry.number,
                start_lba: entry.start_lba,
                end_lba: end_lba.max(entry.start_lba),
                audio,
                pre_emphasis: audio && entry.control & 0x01 != 0,
            });
        }
        Toc {
            first: tracks.first().map_or(1, |t| t.number),
            last: tracks.last().map_or(0, |t| t.number),
            leadout_lba,
            tracks,
        }
    }

    pub fn audio_tracks(&self) -> impl Iterator<Item = &TocTrack> {
        self.tracks.iter().filter(|track| track.audio)
    }

    pub fn track(&self, number: u8) -> Option<&TocTrack> {
        self.tracks.iter().find(|track| track.number == number)
    }

    /// Playing time of the audio tracks.
    pub fn length_ms(&self) -> u64 {
        let sectors: u64 = self.audio_tracks().map(|track| u64::from(track.sectors())).sum();
        sectors * 1000 / u64::from(SECTORS_PER_SECOND)
    }

    /// First sector past the last audio track: what the secure reader treats as the end of the
    /// disc (anything beyond is zero-filled when a read offset asks for it).
    pub fn audio_end_lba(&self) -> u32 {
        self.audio_tracks().map(|track| track.end_lba).max().unwrap_or(0)
    }

    /// Tracks and lead-out as MusicBrainz, CDDB and AccurateRip see them: a trailing data
    /// session (CD-Extra) is left out and the lead-out moves to where the audio session ends.
    fn audio_session(&self) -> (&[TocTrack], u32) {
        let mut count = self.tracks.len();
        while count > 0 && !self.tracks[count - 1].audio {
            count -= 1;
        }
        if count == self.tracks.len() {
            return (&self.tracks, self.leadout_lba);
        }
        let leadout = self.tracks[count].start_lba.saturating_sub(CD_EXTRA_GAP_SECTORS);
        (&self.tracks[..count], leadout)
    }

    /// MusicBrainz disc id: base64 (`._-` alphabet) of SHA-1 over first, last, lead-out and 99
    /// offsets (all +150, upper-case hex).
    pub fn musicbrainz_id(&self) -> String {
        let (tracks, leadout) = self.audio_session();
        let (Some(first), Some(last)) = (tracks.first(), tracks.last()) else {
            return String::new();
        };
        let mut text = String::with_capacity(4 + 8 * 100);
        let _ = write!(text, "{:02X}{:02X}{:08X}", first.number, last.number, leadout + PREGAP_SECTORS);
        for slot in 1..=MAX_TRACKS {
            let offset = tracks.iter().find(|t| t.number == slot).map_or(0, |t| t.start_lba + PREGAP_SECTORS);
            let _ = write!(text, "{offset:08X}");
        }
        let digest = Sha1::digest(text.as_bytes());
        base64::engine::general_purpose::STANDARD
            .encode(digest)
            .chars()
            .map(|c| match c {
                '+' => '.',
                '/' => '_',
                '=' => '-',
                other => other,
            })
            .collect()
    }

    /// The table of contents the way MusicBrainz's `toc=` lookup wants it: first and last track,
    /// the lead-out and every track start, all in sectors with the 2-second pre-gap added,
    /// separated by `+`. Empty when the disc has no audio.
    pub fn musicbrainz_toc(&self) -> String {
        let (tracks, leadout) = self.audio_session();
        let (Some(first), Some(last)) = (tracks.first(), tracks.last()) else {
            return String::new();
        };
        let mut parts = vec![first.number.to_string(), last.number.to_string(), (leadout + PREGAP_SECTORS).to_string()];
        parts.extend(tracks.iter().map(|t| (t.start_lba + PREGAP_SECTORS).to_string()));
        parts.join("+")
    }

    /// CDDB / freeDB id as 8 hex digits.
    pub fn freedb_id(&self) -> String {
        let (tracks, leadout) = self.audio_session();
        let Some(first) = tracks.first() else {
            return String::new();
        };
        let digit_sum = |mut n: u32| {
            let mut sum = 0;
            while n > 0 {
                sum += n % 10;
                n /= 10;
            }
            sum
        };
        let seconds = |lba: u32| (lba + PREGAP_SECTORS) / SECTORS_PER_SECOND;
        let checksum: u32 = tracks.iter().map(|t| digit_sum(seconds(t.start_lba))).sum();
        let total = seconds(leadout) - seconds(first.start_lba);
        let id = (checksum % 255) << 24 | total << 8 | tracks.len() as u32;
        format!("{id:08x}")
    }

    /// CUETools database table of contents: 0-based start of every audio track and the
    /// lead-out, separated by colons.
    pub fn ctdb_toc(&self) -> String {
        let (tracks, leadout) = self.audio_session();
        let mut parts: Vec<String> = tracks.iter().map(|t| t.start_lba.to_string()).collect();
        if !tracks.is_empty() {
            parts.push(leadout.to_string());
        }
        parts.join(":")
    }

    /// AccurateRip `(id1, id2, cddb)`: id1 sums the 0-based offsets (lead-out included), id2 sums
    /// them weighted by position (an offset of 0 counts as 1), cddb is the freeDB id.
    pub fn accuraterip_ids(&self) -> (u32, u32, u32) {
        let (tracks, leadout) = self.audio_session();
        let offsets = tracks.iter().map(|t| t.start_lba).chain(std::iter::once(leadout));
        let (mut id1, mut id2) = (0u32, 0u32);
        for (i, offset) in offsets.enumerate() {
            id1 = id1.wrapping_add(offset);
            id2 = id2.wrapping_add(offset.max(1).wrapping_mul(i as u32 + 1));
        }
        let cddb = u32::from_str_radix(&self.freedb_id(), 16).unwrap_or(0);
        (id1, id2, cddb)
    }

    /// Stereo frames in a track (what the reader delivers).
    pub fn track_frames(&self, number: u8) -> Option<u64> {
        self.track(number).map(|t| u64::from(t.sectors()) * FRAMES_PER_SECTOR as u64)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// 12 audio tracks, lead-out 267 257 (+150): the disc in libdiscid's documentation.
    pub fn libdiscid_example() -> Toc {
        let starts = [150, 22767, 41887, 58317, 72102, 91375, 104652, 115380, 132165, 143932, 159870, 174597];
        let entries: Vec<RawTocEntry> = starts
            .iter()
            .enumerate()
            .map(|(i, start)| RawTocEntry { number: i as u8 + 1, control: 0, start_lba: start - 150 })
            .collect();
        Toc::from_entries(&entries, 267257 - 150)
    }

    #[test]
    fn musicbrainz_id_matches_the_documented_example() {
        assert_eq!(libdiscid_example().musicbrainz_id(), "I5l9cCSFccLKFEKS.7wqSZAorPU-");
    }

    #[test]
    fn freedb_id_follows_the_cddb_algorithm() {
        // Worked by hand: digit sums of the offsets in seconds, mod 255, in the top byte.
        let toc = libdiscid_example();
        assert_eq!(toc.freedb_id(), "a70de90c");
        assert!(toc.freedb_id().ends_with("0c")); // 12 tracks
    }

    #[test]
    fn musicbrainz_toc_is_the_libdiscid_layout() {
        assert_eq!(
            libdiscid_example().musicbrainz_toc(),
            "1+12+267257+150+22767+41887+58317+72102+91375+104652+115380+132165+143932+159870+174597"
        );
    }

    #[test]
    fn ctdb_toc_lists_starts_and_leadout() {
        let toc = libdiscid_example();
        assert!(toc.ctdb_toc().starts_with("0:22617:"));
        assert!(toc.ctdb_toc().ends_with(":267107"));
    }

    #[test]
    fn accuraterip_ids_for_a_known_layout() {
        // Two tracks at 0 and 1000, lead-out 3000: id1 = 4000, id2 = 1*1 + 1000*2 + 3000*3.
        let toc = Toc::from_entries(
            &[
                RawTocEntry { number: 1, control: 0, start_lba: 0 },
                RawTocEntry { number: 2, control: 0, start_lba: 1000 },
            ],
            3000,
        );
        let (id1, id2, _) = toc.accuraterip_ids();
        assert_eq!(id1, 4000);
        assert_eq!(id2, 1 + 2000 + 9000);
    }

    #[test]
    fn length_is_the_audio_time() {
        let toc = Toc::from_entries(&[RawTocEntry { number: 1, control: 0, start_lba: 0 }], 75 * 60);
        assert_eq!(toc.length_ms(), 60_000);
    }

    #[test]
    fn cd_extra_leaves_the_data_session_out() {
        let audio = |n, start| RawTocEntry { number: n, control: 0, start_lba: start };
        let data = RawTocEntry { number: 3, control: 0x04, start_lba: 50_000 };
        let toc = Toc::from_entries(&[audio(1, 0), audio(2, 20_000), data], 60_000);
        assert_eq!(toc.track(2).unwrap().end_lba, 50_000 - CD_EXTRA_GAP_SECTORS);
        assert!(!toc.track(3).unwrap().audio);
        assert_eq!(toc.audio_tracks().count(), 2);
        assert_eq!(toc.audio_end_lba(), 50_000 - CD_EXTRA_GAP_SECTORS);
        // The ids are those of the audio session alone.
        let plain = Toc::from_entries(&[audio(1, 0), audio(2, 20_000)], 50_000 - CD_EXTRA_GAP_SECTORS);
        assert_eq!(toc.musicbrainz_id(), plain.musicbrainz_id());
        assert_eq!(toc.freedb_id(), plain.freedb_id());
        assert_eq!(toc.ctdb_toc(), plain.ctdb_toc());
    }

    #[test]
    fn mixed_mode_keeps_the_leading_data_track() {
        let toc = Toc::from_entries(
            &[
                RawTocEntry { number: 1, control: 0x04, start_lba: 0 },
                RawTocEntry { number: 2, control: 0x01, start_lba: 5000 },
            ],
            9000,
        );
        assert_eq!(toc.audio_tracks().count(), 1);
        assert!(toc.track(2).unwrap().pre_emphasis);
        assert_eq!(toc.track(1).unwrap().end_lba, 5000);
    }

    #[test]
    fn empty_toc_has_no_ids() {
        let toc = Toc::from_entries(&[], 0);
        assert_eq!(toc.musicbrainz_id(), "");
        assert_eq!(toc.length_ms(), 0);
    }
}
