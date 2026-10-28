//! Crossfade: the next song starts while the current one fades out.
//!
//! The settings are what the apps persist (camelCase JSON, the same on the
//! desktop and Android) and the math every platform shares: the gain curves,
//! when a transition fades and when it stays gapless (an album playing in
//! order). Off by default: bit-perfect, gapless playback stays the default.

use serde::{Deserialize, Deserializer, Serialize};

use super::taste::{ItemRef, NowPlayingContext};

pub const MIN_SECONDS: f32 = 1.0;
pub const MAX_SECONDS: f32 = 12.0;
pub const DEFAULT_SECONDS: f32 = 6.0;
/// The quick fade when the user skips or picks a song.
pub const MIN_MANUAL_SECONDS: f32 = 0.5;
pub const MAX_MANUAL_SECONDS: f32 = 3.0;
pub const DEFAULT_MANUAL_SECONDS: f32 = 1.5;
/// Longest "shortest track that fades" the user can set.
pub const MAX_MIN_TRACK_SECONDS: f32 = 120.0;
/// Below this a sample counts as silence for the smart trim (-60 dBFS).
pub const SILENCE_THRESHOLD: f32 = 0.001;
/// The smart trim never removes more than this from either end (a hidden
/// track after minutes of silence keeps its silence).
pub const MAX_TRIM_SECONDS: f32 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum CrossfadeMode {
    /// Gapless, bit-perfect transitions (the default).
    #[default]
    Off,
    On,
}

/// How the two songs' levels move during the fade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum FadeCurve {
    /// Constant power (`cos` / `sin`): the loudness stays even through the fade.
    #[default]
    EqualPower,
    /// Straight lines: a slight dip in the middle with uncorrelated songs.
    Linear,
    /// Equal power on an eased clock: lingers on each song, moves fast in the middle.
    SCurve,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CrossfadeSettings {
    #[serde(deserialize_with = "lenient_mode")]
    pub mode: CrossfadeMode,
    /// Length of the fade between songs, 1..=12 s.
    pub seconds: f32,
    #[serde(deserialize_with = "lenient_curve")]
    pub curve: FadeCurve,
    /// Fade from where the music ends (trailing silence below -60 dBFS
    /// skipped) into where the next one starts (leading silence skipped).
    pub smart: bool,
    /// Consecutive tracks of the same album, in order, stay gapless.
    pub skip_album_runs: bool,
    /// A short fade when the user presses next / previous or picks a song.
    pub on_manual_skip: bool,
    /// Length of that fade, 0.5..=3 s.
    pub manual_seconds: f32,
    /// Tracks shorter than this never fade (0 = twice the fade).
    pub min_track_seconds: f32,
    /// Android: fall back to normal transitions on low-memory phones or when
    /// the phone can't keep up with two players.
    pub low_power_fallback: bool,
}

impl Default for CrossfadeSettings {
    fn default() -> Self {
        Self {
            mode: CrossfadeMode::Off,
            seconds: DEFAULT_SECONDS,
            curve: FadeCurve::EqualPower,
            smart: true,
            skip_album_runs: true,
            on_manual_skip: true,
            manual_seconds: DEFAULT_MANUAL_SECONDS,
            min_track_seconds: 0.0,
            low_power_fallback: true,
        }
    }
}

impl CrossfadeSettings {
    pub fn is_on(&self) -> bool {
        self.mode == CrossfadeMode::On
    }

    /// Values in range (NaN reads as the default).
    pub fn clamped(&self) -> Self {
        let pick = |v: f32, lo: f32, hi: f32, nan: f32| if v.is_finite() { v.clamp(lo, hi) } else { nan };
        Self {
            seconds: pick(self.seconds, MIN_SECONDS, MAX_SECONDS, DEFAULT_SECONDS),
            manual_seconds: pick(self.manual_seconds, MIN_MANUAL_SECONDS, MAX_MANUAL_SECONDS, DEFAULT_MANUAL_SECONDS),
            min_track_seconds: pick(self.min_track_seconds, 0.0, MAX_MIN_TRACK_SECONDS, 0.0),
            ..*self
        }
    }

    /// Fade length in seconds for a natural transition, or a skip (`manual`).
    pub fn fade_seconds(&self, manual: bool) -> f32 {
        let s = self.clamped();
        if manual {
            s.manual_seconds.min(s.seconds.max(MIN_MANUAL_SECONDS))
        } else {
            s.seconds
        }
    }

    /// Shortest track (seconds) that fades with a `fade` second fade.
    pub fn min_track(&self, fade: f32) -> f32 {
        let s = self.clamped();
        if s.min_track_seconds > 0.0 {
            s.min_track_seconds
        } else {
            2.0 * fade
        }
    }

    /// `true` when a track of `duration_ms` may take part in a `fade` second
    /// fade (unknown lengths may: the engine shortens the fade if it must).
    pub fn long_enough(&self, duration_ms: Option<u64>, fade: f32) -> bool {
        duration_ms.is_none_or(|ms| ms as f32 / 1000.0 >= self.min_track(fade))
    }

    /// Whether going from `from` to `to` fades (crossfade on, not an album
    /// run when those stay gapless, both long enough). `manual`: the user
    /// skipped or picked the song.
    pub fn fades(&self, from: &TrackInfo, to: &TrackInfo, shuffle: bool, manual: bool) -> bool {
        if !self.is_on() || manual && !self.on_manual_skip {
            return false;
        }
        if !manual && self.skip_album_runs && is_album_run(from.album, from.artist, to.album, to.artist, shuffle) {
            return false;
        }
        let fade = self.fade_seconds(manual);
        // a skip leaves the rest of the outgoing track: only the incoming one must be long enough
        (manual || self.long_enough(from.duration_ms, fade)) && self.long_enough(to.duration_ms, fade)
    }
}

/// What [`CrossfadeSettings::fades`] needs to know about a track.
#[derive(Debug, Clone, Copy, Default)]
pub struct TrackInfo<'a> {
    pub album: Option<&'a ItemRef>,
    /// Main (album) artist.
    pub artist: Option<&'a ItemRef>,
    pub duration_ms: Option<u64>,
}

impl<'a> TrackInfo<'a> {
    pub fn of(ctx: &'a NowPlayingContext, duration_ms: Option<u64>) -> Self {
        Self { album: ctx.album.as_ref(), artist: ctx.main_artist(), duration_ms }
    }
}

/// Two consecutive tracks of one album, in order: they stay gapless (live
/// albums, DJ mixes and concept albums flow into each other). Album ids
/// decide when both have one; by name, the main artist must match too
/// (two "Greatest Hits" are different albums), unless one is unknown.
pub fn is_album_run(
    album_a: Option<&ItemRef>,
    artist_a: Option<&ItemRef>,
    album_b: Option<&ItemRef>,
    artist_b: Option<&ItemRef>,
    shuffle: bool,
) -> bool {
    let (Some(a), Some(b)) = (album_a, album_b) else { return false };
    if shuffle || a.is_empty() || b.is_empty() {
        return false;
    }
    if !a.id.trim().is_empty() && !b.id.trim().is_empty() {
        return a.id.trim() == b.id.trim();
    }
    if !a.matches(b) {
        return false;
    }
    match (artist_a.filter(|x| !x.is_empty()), artist_b.filter(|x| !x.is_empty())) {
        (Some(x), Some(y)) => x.matches(y),
        _ => true,
    }
}

/// Gains `(outgoing, incoming)` at `t` (0 = fade starts, 1 = done), each 0..=1.
pub fn gains(curve: FadeCurve, t: f32) -> (f32, f32) {
    let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
    let quarter = std::f32::consts::FRAC_PI_2;
    match curve {
        FadeCurve::Linear => (1.0 - t, t),
        FadeCurve::EqualPower => ((t * quarter).cos(), (t * quarter).sin()),
        FadeCurve::SCurve => {
            let eased = t * t * (3.0 - 2.0 * t);
            ((eased * quarter).cos(), (eased * quarter).sin())
        }
    }
}

/// [`CrossfadeSettings`] falling back to the defaults (off) when unreadable.
pub fn lenient_crossfade<'de, D: Deserializer<'de>>(d: D) -> Result<CrossfadeSettings, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Any {
        Known(CrossfadeSettings),
        Other(serde::de::IgnoredAny),
    }
    Ok(match Any::deserialize(d)? {
        Any::Known(s) => s.clamped(),
        Any::Other(_) => CrossfadeSettings::default(),
    })
}

fn lenient_mode<'de, D: Deserializer<'de>>(d: D) -> Result<CrossfadeMode, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Any {
        Known(CrossfadeMode),
        Other(serde::de::IgnoredAny),
    }
    Ok(match Any::deserialize(d)? {
        Any::Known(m) => m,
        Any::Other(_) => CrossfadeMode::Off,
    })
}

fn lenient_curve<'de, D: Deserializer<'de>>(d: D) -> Result<FadeCurve, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Any {
        Known(FadeCurve),
        Other(serde::de::IgnoredAny),
    }
    Ok(match Any::deserialize(d)? {
        Any::Known(c) => c,
        Any::Other(_) => FadeCurve::EqualPower,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CURVES: [FadeCurve; 3] = [FadeCurve::EqualPower, FadeCurve::Linear, FadeCurve::SCurve];

    #[test]
    fn curves_start_on_the_outgoing_song_and_end_on_the_incoming_one() {
        for curve in CURVES {
            let (o, i) = gains(curve, 0.0);
            assert!((o - 1.0).abs() < 1e-6 && i.abs() < 1e-6, "{curve:?}");
            let (o, i) = gains(curve, 1.0);
            assert!(o.abs() < 1e-6 && (i - 1.0).abs() < 1e-6, "{curve:?}");
            // out of range and NaN clamp
            assert_eq!(gains(curve, -1.0), gains(curve, 0.0));
            assert_eq!(gains(curve, 7.0), gains(curve, 1.0));
            assert_eq!(gains(curve, f32::NAN), gains(curve, 0.0));
            // monotonic: one goes down while the other goes up
            let mut last = gains(curve, 0.0);
            for k in 1..=100 {
                let g = gains(curve, k as f32 / 100.0);
                assert!(g.0 <= last.0 + 1e-6 && g.1 >= last.1 - 1e-6, "{curve:?} at {k}");
                last = g;
            }
        }
    }

    #[test]
    fn equal_power_keeps_the_loudness_even() {
        // uncorrelated songs add in power: out² + in² must stay 1 (0 dB) all the way
        for curve in [FadeCurve::EqualPower, FadeCurve::SCurve] {
            for k in 0..=1000 {
                let (o, i) = gains(curve, k as f32 / 1000.0);
                assert!((o * o + i * i - 1.0).abs() < 1e-5, "{curve:?} at {k}");
            }
        }
        // linear keeps the amplitude sum instead, and dips 3 dB in power halfway
        let (o, i) = gains(FadeCurve::Linear, 0.5);
        assert!((o + i - 1.0).abs() < 1e-6);
        let dip_db = 10.0 * (o * o + i * i).log10();
        assert!((dip_db + 3.01).abs() < 0.05, "{dip_db}");
    }

    #[test]
    fn s_curve_lingers_at_the_ends() {
        let (_, eq) = gains(FadeCurve::EqualPower, 0.1);
        let (_, s) = gains(FadeCurve::SCurve, 0.1);
        assert!(s < eq, "the incoming song comes in later: {s} vs {eq}");
        let (o, i) = gains(FadeCurve::SCurve, 0.5);
        assert!((o - i).abs() < 1e-6, "same level halfway");
    }

    #[test]
    fn settings_default_off_and_clamp() {
        let s = CrossfadeSettings::default();
        assert!(!s.is_on());
        assert_eq!(
            (s.seconds, s.curve, s.smart, s.skip_album_runs, s.on_manual_skip),
            (6.0, FadeCurve::EqualPower, true, true, true)
        );
        let wild =
            CrossfadeSettings { seconds: 99.0, manual_seconds: f32::NAN, min_track_seconds: -3.0, ..s }.clamped();
        assert_eq!((wild.seconds, wild.manual_seconds, wild.min_track_seconds), (12.0, 1.5, 0.0));
        // a skip never fades longer than the normal fade
        let short = CrossfadeSettings { seconds: 1.0, manual_seconds: 3.0, ..s };
        assert_eq!(short.fade_seconds(true), 1.0);
        assert_eq!(s.fade_seconds(true), 1.5);
        // auto minimum: twice the fade
        assert_eq!(s.min_track(6.0), 12.0);
        assert_eq!(CrossfadeSettings { min_track_seconds: 30.0, ..s }.min_track(6.0), 30.0);
    }

    #[test]
    fn json_is_camel_case_and_lenient() {
        let on = CrossfadeSettings { mode: CrossfadeMode::On, curve: FadeCurve::SCurve, ..Default::default() };
        let json = serde_json::to_string(&on).unwrap();
        assert!(
            json.contains(r#""mode":"on""#) && json.contains(r#""curve":"sCurve""#) && json.contains("skipAlbumRuns"),
            "{json}"
        );
        assert_eq!(serde_json::from_str::<CrossfadeSettings>(&json).unwrap(), on);
        // unknown values from a newer version: off / equal power, the rest kept
        let odd: CrossfadeSettings = serde_json::from_str(r#"{"mode":"dj","curve":"log","seconds":4}"#).unwrap();
        assert_eq!((odd.mode, odd.curve, odd.seconds), (CrossfadeMode::Off, FadeCurve::EqualPower, 4.0));
        #[derive(Deserialize)]
        struct Holder {
            #[serde(deserialize_with = "lenient_crossfade")]
            crossfade: CrossfadeSettings,
        }
        let bad: Holder = serde_json::from_str(r#"{"crossfade":{"seconds":"long"}}"#).unwrap();
        assert_eq!(bad.crossfade, CrossfadeSettings::default());
    }

    #[test]
    fn album_runs_stay_gapless() {
        let album = |id: &str, name: &str| ItemRef::new(id, name);
        let artist = ItemRef::named("Pink Floyd");
        let other = ItemRef::named("Queen");
        let dsotm = album("", "The Dark Side of the Moon");
        // same album by name and artist, in order
        assert!(is_album_run(Some(&dsotm), Some(&artist), Some(&dsotm), Some(&artist), false));
        // shuffled: a fade
        assert!(!is_album_run(Some(&dsotm), Some(&artist), Some(&dsotm), Some(&artist), true));
        // same name, another artist: another album
        let hits = album("", "Greatest Hits");
        assert!(!is_album_run(Some(&hits), Some(&artist), Some(&hits), Some(&other), false));
        // ids decide when both have one (a compilation keeps its id across artists)
        assert!(is_album_run(
            Some(&album("42", "Now 1")),
            Some(&artist),
            Some(&album("42", "Now 1")),
            Some(&other),
            false
        ));
        assert!(!is_album_run(Some(&album("42", "X")), None, Some(&album("43", "X")), None, false));
        // unknown albums never count
        assert!(!is_album_run(None, None, Some(&dsotm), None, false));
        assert!(!is_album_run(Some(&ItemRef::named(" ")), None, Some(&ItemRef::named(" ")), None, false));
    }

    #[test]
    fn which_transitions_fade() {
        let on = CrossfadeSettings { mode: CrossfadeMode::On, ..Default::default() };
        let a = ItemRef::named("Album");
        let b = ItemRef::named("Other");
        fn t(album: &ItemRef, ms: u64) -> TrackInfo<'_> {
            TrackInfo { album: Some(album), artist: None, duration_ms: Some(ms) }
        }
        let (x, y, z) = (t(&a, 200_000), t(&a, 180_000), t(&b, 180_000));
        assert!(!on.fades(&x, &y, false, false), "album run: gapless");
        assert!(on.fades(&x, &y, true, false), "shuffled: fades");
        assert!(on.fades(&x, &z, false, false));
        assert!(CrossfadeSettings { skip_album_runs: false, ..on }.fades(&x, &y, false, false));
        // a skip fades even inside an album
        assert!(on.fades(&x, &y, false, true));
        assert!(!CrossfadeSettings { on_manual_skip: false, ..on }.fades(&x, &z, false, true));
        // too short: 10 s < 2 × 6 s
        assert!(!on.fades(&x, &t(&b, 10_000), false, false));
        assert!(!on.fades(&t(&b, 10_000), &x, false, false));
        // a skip only needs the incoming song to be long enough (2 × 1.5 s)
        assert!(on.fades(&t(&b, 10_000), &x, false, true));
        // unknown lengths may fade
        assert!(on.fades(&TrackInfo { duration_ms: None, ..x }, &z, false, false));
        assert!(!CrossfadeSettings::default().fades(&x, &z, false, false), "off");
    }
}
