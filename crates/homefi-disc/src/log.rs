//! The rip log, written next to the songs: what drive and settings were used and, per track,
//! how the reading went. Plain text, EAC-like, so two rips of a disc can be compared.

use std::fmt::Write;

use crate::encode::{RipFormat, Verified};
use crate::read::{ReadMode, ReadPlan, TrackRead};

/// How one track ended.
#[derive(Debug, Clone)]
pub struct TrackLog {
    pub number: u8,
    pub title: String,
    pub outcome: TrackOutcome,
}

#[derive(Debug, Clone)]
pub enum TrackOutcome {
    Saved { file: String, read: TrackRead, crc32: u32, verified: Option<Verified> },
    Failed(String),
}

pub struct RipLog<'a> {
    pub app_version: &'a str,
    pub when: &'a str,
    pub drive: &'a str,
    pub artist: &'a str,
    pub album: &'a str,
    pub format: &'a RipFormat,
    pub plan: &'a ReadPlan,
    pub tracks: &'a [TrackLog],
}

fn mode_text(mode: ReadMode) -> &'static str {
    match mode {
        ReadMode::Secure => "secure (every sector read until identical reads agree)",
        ReadMode::Fast => "fast (one read, repeated only when the drive flags an error)",
    }
}

/// Renders the log.
pub fn render(log: &RipLog<'_>) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "HomeFi {} rip log", log.app_version);
    let _ = writeln!(out, "{}\n", log.when);
    let _ = writeln!(out, "Album          : {} / {}", log.artist, log.album);
    let _ = writeln!(out, "Drive          : {}", log.drive);
    let _ = writeln!(out, "Read offset    : {:+} samples", log.plan.offset_samples);
    let _ = writeln!(out, "Read mode      : {}", mode_text(log.plan.mode));
    if log.plan.mode == ReadMode::Secure {
        let _ = writeln!(out, "Reads to agree : {}", log.plan.agree);
    }
    let _ = writeln!(out, "Max re-reads   : {} per sector", log.plan.max_rereads);
    let _ = writeln!(out, "Format         : {}\n", log.format.label());
    for track in log.tracks {
        let _ = writeln!(out, "Track {:02}  {}", track.number, track.title);
        match &track.outcome {
            TrackOutcome::Saved { file, read, crc32, verified } => {
                let _ = writeln!(out, "    File               : {file}");
                let _ = writeln!(out, "    Audio CRC32        : {crc32:08X}");
                let _ = writeln!(out, "    Re-reads           : {}", read.rereads);
                let _ = writeln!(out, "    C2 errors          : {}", read.c2_errors);
                let _ = writeln!(out, "    Doubtful samples   : {}", read.suspicious_samples);
                let _ = writeln!(out, "    Unreadable samples : {}", read.unreadable_samples);
                if matches!(verified, Some(Verified::FlacDecoded)) {
                    let _ = writeln!(out, "    FLAC check         : decoded again, identical");
                }
                let verdict = if read.unreadable_samples > 0 {
                    "damaged: some samples could not be read"
                } else if read.suspicious_samples > 0 {
                    "doubtful: some samples could not be confirmed"
                } else {
                    "read without errors"
                };
                let _ = writeln!(out, "    Result             : {verdict}\n");
            }
            TrackOutcome::Failed(message) => {
                let _ = writeln!(out, "    Not saved          : {message}\n");
            }
        }
    }
    let saved = log.tracks.iter().filter(|t| matches!(t.outcome, TrackOutcome::Saved { .. })).count();
    let _ = writeln!(out, "{saved} of {} tracks saved", log.tracks.len());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_says_how_each_track_went() {
        let plan = ReadPlan { offset_samples: 6, ..ReadPlan::default() };
        let tracks = vec![
            TrackLog {
                number: 1,
                title: "Blind".into(),
                outcome: TrackOutcome::Saved {
                    file: "Blind.flac".into(),
                    read: TrackRead { rereads: 3, ..TrackRead::default() },
                    crc32: 0xDEADBEEF,
                    verified: Some(Verified::FlacDecoded),
                },
            },
            TrackLog { number: 2, title: "Ball Tongue".into(), outcome: TrackOutcome::Failed("scratched".into()) },
        ];
        let text = render(&RipLog {
            app_version: "1.5.2",
            when: "2026-11-04 12:00",
            drive: "HL-DT-ST BD-RE",
            artist: "Korn",
            album: "Korn",
            format: &RipFormat::default(),
            plan: &plan,
            tracks: &tracks,
        });
        assert!(text.contains("+6 samples"));
        assert!(text.contains("DEADBEEF"));
        assert!(text.contains("decoded again, identical"));
        assert!(text.contains("Not saved          : scratched"));
        assert!(text.contains("1 of 2 tracks saved"));
    }
}
