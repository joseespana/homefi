//! Loudness normalization from ReplayGain: Tidal's track / album gain and
//! peak, or the `REPLAYGAIN_*` (and Opus `R128_*`) tags of local files.
//!
//! Off by default: any gain other than 0 dB means the path is no longer
//! bit-perfect. With `prevent_clipping` a boost never pushes the track's
//! peak over full scale; the equalizer's automatic preamp leaves room for its
//! own boosts, so the two together never clip.

use std::path::Path;

use serde::{Deserialize, Deserializer, Serialize};

/// Loudness ReplayGain 2.0 values are relative to.
pub const REPLAYGAIN_REFERENCE_LUFS: f32 = -18.0;
/// Opus' `R128_*_GAIN` tags are relative to this.
const R128_REFERENCE_LUFS: f32 = -23.0;
pub const MIN_TARGET_LUFS: f32 = -24.0;
pub const MAX_TARGET_LUFS: f32 = -8.0;
/// Normalization never changes the level by more than this.
const MAX_CUT_DB: f32 = -24.0;
const MAX_BOOST_DB: f32 = 12.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum LoudnessMode {
    #[default]
    Off,
    /// Every track at the same loudness.
    Track,
    /// Albums keep their own dynamics (quiet songs stay quieter).
    Album,
    /// Album gain while an album plays in order, track gain otherwise.
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LoudnessSettings {
    #[serde(deserialize_with = "lenient_mode")]
    pub mode: LoudnessMode,
    /// Loudness to bring music to, -24..=-8 LUFS (streaming services use -14).
    pub target_lufs: f32,
    /// Never boost a track past its peak.
    pub prevent_clipping: bool,
}

impl Default for LoudnessSettings {
    fn default() -> Self {
        Self { mode: LoudnessMode::Off, target_lufs: -14.0, prevent_clipping: true }
    }
}

/// ReplayGain of a track: gains in dB (to reach [`REPLAYGAIN_REFERENCE_LUFS`]),
/// peaks as linear sample amplitude (1.0 = full scale).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ReplayGain {
    pub track_gain_db: Option<f32>,
    pub track_peak: Option<f32>,
    pub album_gain_db: Option<f32>,
    pub album_peak: Option<f32>,
}

impl ReplayGain {
    pub fn is_empty(&self) -> bool {
        self.track_gain_db.is_none() && self.album_gain_db.is_none()
    }

    /// From the track listing (`replayGain`, `peak`).
    pub fn from_track(track: &crate::models::Track) -> Self {
        Self {
            track_gain_db: track.replay_gain.map(|g| g as f32).filter(|g| g.is_finite()),
            track_peak: track.peak.map(|p| p as f32).filter(|p| p.is_finite() && *p > 0.0),
            ..Default::default()
        }
    }

    /// From `playbackinfo` (`trackReplayGain`, `trackPeakAmplitude`, `albumReplayGain`, `albumPeakAmplitude`).
    pub fn from_stream(stream: &crate::models::TrackStream) -> Self {
        let gain = |v: Option<f64>| v.map(|g| g as f32).filter(|g| g.is_finite());
        let peak = |v: Option<f64>| v.map(|p| p as f32).filter(|p| p.is_finite() && *p > 0.0);
        Self {
            track_gain_db: gain(stream.track_replay_gain),
            track_peak: peak(stream.track_peak_amplitude),
            album_gain_db: gain(stream.album_replay_gain),
            album_peak: peak(stream.album_peak_amplitude),
        }
    }

    /// `self`, with what it lacks taken from `other`.
    pub fn or(self, other: ReplayGain) -> Self {
        Self {
            track_gain_db: self.track_gain_db.or(other.track_gain_db),
            track_peak: self.track_peak.or(other.track_peak),
            album_gain_db: self.album_gain_db.or(other.album_gain_db),
            album_peak: self.album_peak.or(other.album_peak),
        }
    }
}

impl LoudnessSettings {
    pub fn is_on(&self) -> bool {
        self.mode != LoudnessMode::Off
    }

    /// Values in range.
    pub fn clamped(&self) -> Self {
        let target = if self.target_lufs.is_finite() { self.target_lufs } else { -14.0 };
        Self { target_lufs: target.clamp(MIN_TARGET_LUFS, MAX_TARGET_LUFS), ..*self }
    }

    /// Whether album gain is used: `album_in_order` says an album is playing
    /// in its own order (for [`LoudnessMode::Auto`]).
    pub fn uses_album(&self, album_in_order: bool) -> bool {
        match self.mode {
            LoudnessMode::Album => true,
            LoudnessMode::Auto => album_in_order,
            LoudnessMode::Off | LoudnessMode::Track => false,
        }
    }

    /// Gain in dB to apply to a track with `rg` (0 when off or unknown),
    /// rounded to 0.01 dB. With `prevent_clipping`, a boost stops at the
    /// peak; without a known peak there is no boost at all.
    pub fn gain_db(&self, rg: &ReplayGain, album_in_order: bool) -> f32 {
        if !self.is_on() {
            return 0.0;
        }
        let s = self.clamped();
        let album = s.uses_album(album_in_order);
        let (gain, peak) = match (album, rg.album_gain_db, rg.track_gain_db) {
            (true, Some(g), _) => (g, rg.album_peak.or(rg.track_peak)),
            (_, _, Some(g)) => (g, rg.track_peak.or(rg.album_peak)),
            (false, Some(g), None) => (g, rg.album_peak),
            (_, None, None) => return 0.0,
        };
        let mut db = (gain + s.target_lufs - REPLAYGAIN_REFERENCE_LUFS).clamp(MAX_CUT_DB, MAX_BOOST_DB);
        if s.prevent_clipping && db > 0.0 {
            let headroom = match peak.filter(|p| *p > 0.0 && p.is_finite()) {
                Some(p) => -20.0 * p.log10(),
                None => 0.0,
            };
            db = db.min(headroom.max(0.0));
        }
        (db * 100.0).round() / 100.0
    }
}

/// `-6.20 dB`, `+1.5dB`, `-3,2 dB` → dB.
pub fn parse_gain(text: &str) -> Option<f32> {
    let t = text.trim().trim_end_matches(|c: char| c.is_alphabetic() || c.is_whitespace()).replace(',', ".");
    t.trim().trim_start_matches('+').parse::<f32>().ok().filter(|g| g.is_finite() && g.abs() < 60.0)
}

/// `0.988525` → linear peak.
pub fn parse_peak(text: &str) -> Option<f32> {
    text.trim().replace(',', ".").parse::<f32>().ok().filter(|p| p.is_finite() && *p > 0.0 && *p < 100.0)
}

/// An Opus `R128_*_GAIN` (Q7.8 fixed point, relative to -23 LUFS) as a ReplayGain dB.
pub fn r128_to_replaygain(q78: &str) -> Option<f32> {
    let v = q78.trim().parse::<i32>().ok()?;
    Some(v as f32 / 256.0 + (R128_REFERENCE_LUFS - REPLAYGAIN_REFERENCE_LUFS))
}

/// ReplayGain of a local file from its tags; `None` when it has none.
pub fn read_replay_gain(path: &Path) -> Option<ReplayGain> {
    use lofty::config::ParseOptions;
    use lofty::file::TaggedFileExt;
    use lofty::probe::Probe;
    use lofty::tag::ItemKey;

    let tagged = Probe::open(path)
        .ok()?
        .options(ParseOptions::new().read_cover_art(false).read_properties(false))
        .guess_file_type()
        .ok()?
        .read()
        .ok()?;
    let mut rg = ReplayGain::default();
    for tag in tagged.tags() {
        let mut found = ReplayGain::default();
        let mut r128 = (None, None);
        for item in tag.items() {
            let Some(text) = item.value().text() else { continue };
            let unknown = |name: &str| matches!(item.key(), ItemKey::Unknown(k) if k.eq_ignore_ascii_case(name));
            match item.key() {
                ItemKey::ReplayGainTrackGain => found.track_gain_db = found.track_gain_db.or(parse_gain(text)),
                ItemKey::ReplayGainTrackPeak => found.track_peak = found.track_peak.or(parse_peak(text)),
                ItemKey::ReplayGainAlbumGain => found.album_gain_db = found.album_gain_db.or(parse_gain(text)),
                ItemKey::ReplayGainAlbumPeak => found.album_peak = found.album_peak.or(parse_peak(text)),
                _ if unknown("REPLAYGAIN_TRACK_GAIN") => found.track_gain_db = parse_gain(text),
                _ if unknown("REPLAYGAIN_TRACK_PEAK") => found.track_peak = parse_peak(text),
                _ if unknown("REPLAYGAIN_ALBUM_GAIN") => found.album_gain_db = parse_gain(text),
                _ if unknown("REPLAYGAIN_ALBUM_PEAK") => found.album_peak = parse_peak(text),
                _ if unknown("R128_TRACK_GAIN") => r128.0 = r128_to_replaygain(text),
                _ if unknown("R128_ALBUM_GAIN") => r128.1 = r128_to_replaygain(text),
                _ => {}
            }
        }
        found.track_gain_db = found.track_gain_db.or(r128.0);
        found.album_gain_db = found.album_gain_db.or(r128.1);
        rg = rg.or(found);
    }
    (!rg.is_empty()).then_some(rg)
}

/// Tag values as written to files: `-6.20 dB`, `0.988525`.
pub fn format_gain(db: f32) -> String {
    format!("{db:+.2} dB")
}

pub fn format_peak(peak: f32) -> String {
    format!("{peak:.6}")
}

fn lenient_mode<'de, D: Deserializer<'de>>(d: D) -> Result<LoudnessMode, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Any {
        Known(LoudnessMode),
        Other(serde::de::IgnoredAny),
    }
    Ok(match Any::deserialize(d)? {
        Any::Known(m) => m,
        Any::Other(_) => LoudnessMode::Off,
    })
}

/// [`LoudnessSettings`] falling back to the defaults (off) when unreadable.
pub fn lenient_loudness<'de, D: Deserializer<'de>>(d: D) -> Result<LoudnessSettings, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Any {
        Known(LoudnessSettings),
        Other(serde::de::IgnoredAny),
    }
    Ok(match Any::deserialize(d)? {
        Any::Known(s) => s,
        Any::Other(_) => LoudnessSettings::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rg(track: f32, peak: f32, album: f32, album_peak: f32) -> ReplayGain {
        ReplayGain {
            track_gain_db: Some(track),
            track_peak: Some(peak),
            album_gain_db: Some(album),
            album_peak: Some(album_peak),
        }
    }

    #[test]
    fn off_by_default_and_json_is_stable() {
        let s = LoudnessSettings::default();
        assert_eq!(s.mode, LoudnessMode::Off);
        assert_eq!(s.gain_db(&rg(-8.0, 1.0, -7.0, 1.0), true), 0.0);
        let json = serde_json::to_value(s).unwrap();
        assert_eq!(json, serde_json::json!({"mode": "off", "targetLufs": -14.0, "preventClipping": true}));
        let odd: LoudnessSettings = serde_json::from_str(r#"{"mode":"smart","targetLufs":-99}"#).unwrap();
        assert_eq!(odd.mode, LoudnessMode::Off);
        assert_eq!(odd.clamped().target_lufs, MIN_TARGET_LUFS);
    }

    #[test]
    fn modes_pick_track_or_album_gain() {
        let loud = rg(-10.0, 1.0, -8.0, 1.0);
        let at = |mode, album| LoudnessSettings { mode, ..Default::default() }.gain_db(&loud, album);
        // -14 LUFS target is 4 dB above the -18 reference
        assert_eq!(at(LoudnessMode::Track, true), -6.0);
        assert_eq!(at(LoudnessMode::Album, false), -4.0);
        assert_eq!(at(LoudnessMode::Auto, true), -4.0);
        assert_eq!(at(LoudnessMode::Auto, false), -6.0);
        // only one kind known: use it
        let only_album = ReplayGain { album_gain_db: Some(-8.0), ..Default::default() };
        assert_eq!(
            LoudnessSettings { mode: LoudnessMode::Track, ..Default::default() }.gain_db(&only_album, false),
            -4.0
        );
        assert_eq!(
            LoudnessSettings { mode: LoudnessMode::Track, ..Default::default() }.gain_db(&ReplayGain::default(), false),
            0.0
        );
    }

    #[test]
    fn boosts_stop_at_the_peak() {
        let quiet = rg(6.0, 0.5, 6.0, 0.5); // a -6 dBFS peak leaves 6.02 dB of room
        let s = LoudnessSettings { mode: LoudnessMode::Track, ..Default::default() };
        assert_eq!(s.gain_db(&quiet, false), 6.02);
        let free = LoudnessSettings { prevent_clipping: false, ..s };
        assert_eq!(free.gain_db(&quiet, false), 10.0);
        // unknown peak: no boost, cuts still apply
        let no_peak = ReplayGain { track_gain_db: Some(3.0), ..Default::default() };
        assert_eq!(s.gain_db(&no_peak, false), 0.0);
        let loud = ReplayGain { track_gain_db: Some(-9.0), ..Default::default() };
        assert_eq!(s.gain_db(&loud, false), -5.0);
    }

    #[test]
    fn tag_values_parse() {
        assert_eq!(parse_gain("-6.20 dB"), Some(-6.2));
        assert_eq!(parse_gain("+1.5dB"), Some(1.5));
        assert_eq!(parse_gain("-3,25 dB"), Some(-3.25));
        assert_eq!(parse_gain("loud"), None);
        assert_eq!(parse_peak("0.988525"), Some(0.988525));
        assert_eq!(parse_peak("-1"), None);
        // R128: -2048 / 256 = -8 dB at -23 LUFS = -13 dB at -18
        assert_eq!(r128_to_replaygain("-2048"), Some(-13.0));
        assert_eq!(format_gain(-6.2), "-6.20 dB");
        assert_eq!(parse_gain(&format_gain(2.0)), Some(2.0));
    }

    #[test]
    fn tidal_values_become_replaygain() {
        let stream: crate::models::TrackStream = serde_json::from_value(serde_json::json!({
            "trackId": 1, "audioQuality": "LOSSLESS", "manifestMimeType": "application/vnd.tidal.bts", "manifest": "",
            "trackReplayGain": -9.43, "trackPeakAmplitude": 0.999969, "albumReplayGain": -8.9, "albumPeakAmplitude": 1.0
        }))
        .unwrap();
        let rg = ReplayGain::from_stream(&stream);
        assert_eq!(rg.track_gain_db, Some(-9.43));
        assert_eq!(rg.album_peak, Some(1.0));
        // older answers without them
        let bare: crate::models::TrackStream = serde_json::from_value(serde_json::json!({
            "trackId": 1, "audioQuality": "LOSSLESS", "manifestMimeType": "x", "manifest": ""
        }))
        .unwrap();
        assert!(ReplayGain::from_stream(&bare).is_empty());
    }
}
