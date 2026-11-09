//! DJ mode options. The same JSON on desktop (`desktop.json`) and Android; lenient like the
//! other settings: an unreadable value falls back to the defaults instead of failing to load.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};

use super::layout::DjLayout;
use super::mixer_math::CrossfaderCurve;

/// Tempo fader range, +/- percent.
pub const MIN_TEMPO_RANGE_PCT: f32 = 4.0;
pub const MAX_TEMPO_RANGE_PCT: f32 = 50.0;
pub const DEFAULT_TEMPO_RANGE_PCT: f32 = 8.0;
/// Pitch bend (nudge) strength, percent of speed.
pub const MIN_BEND_PCT: f32 = 1.0;
pub const MAX_BEND_PCT: f32 = 10.0;
pub const DEFAULT_BEND_PCT: f32 = 4.0;
/// RAM the two decks may hold decoded (MB). Android uses [`ANDROID_MEMORY_MB`].
pub const MIN_MEMORY_MB: u32 = 128;
pub const MAX_MEMORY_MB: u32 = 4096;
pub const DEFAULT_MEMORY_MB: u32 = 1024;
pub const ANDROID_MEMORY_MB: u32 = 256;
/// Largest boost of a channel EQ band (dB).
pub const MIN_EQ_BOOST_DB: f32 = 0.0;
pub const MAX_EQ_BOOST_DB: f32 = 12.0;
pub const DEFAULT_EQ_BOOST_DB: f32 = 6.0;
/// Lengths an automatic transition can have, in beats.
pub const AUTO_MIX_BEATS: [u32; 3] = [8, 16, 32];
pub const DEFAULT_AUTO_MIX_BEATS: u32 = 16;
/// Beats the zoomed waveform shows.
pub const MIN_ZOOM_BEATS: u8 = 2;
pub const MAX_ZOOM_BEATS: u8 = 32;
pub const DEFAULT_ZOOM_BEATS: u8 = 8;
pub const DEFAULT_SAMPLER_VOLUME: f32 = 0.8;

/// Output buffer size: lower is more responsive, higher survives a busy computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DjLatency {
    /// About 256 frames.
    Low,
    /// About 1024 frames.
    Safe,
    /// About 512 frames (`serde(other)` must be last: unknown values land here).
    #[default]
    #[serde(other)]
    Balanced,
}

impl DjLatency {
    /// Frames of the fixed output buffer to ask the device for.
    pub fn frames(self) -> u32 {
        match self {
            Self::Low => 256,
            Self::Balanced => 512,
            Self::Safe => 1024,
        }
    }
}

/// Where a song picked while DJ mode is on goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LoadTarget {
    /// A prompt asks which deck.
    Ask,
    /// The crate (a list of songs to load later).
    Crate,
    /// The deck that is not playing (deck A when both are idle).
    #[default]
    #[serde(other)]
    FreeDeck,
}

/// How big Homifito, the mascot, is on the DJ screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HomifitoMode {
    Off,
    /// At his small DJ booth.
    #[default]
    Booth,
    Big,
}

/// Which edge of the DJ screen Homifito stands at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HomifitoPlace {
    #[default]
    Top,
    Bottom,
}

/// "Homifito DJ": the mascot at his booth on the DJ screen. Unknown values mean the defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct HomifitoDj {
    #[serde(deserialize_with = "lenient_enum")]
    pub mode: HomifitoMode,
    #[serde(deserialize_with = "lenient_enum")]
    pub place: HomifitoPlace,
    /// The board in Homifito's colours.
    #[serde(deserialize_with = "lenient_bool")]
    pub look: bool,
}

/// An enum whose value is unknown (or of another type) becomes its default.
fn lenient_enum<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + serde::de::DeserializeOwned,
{
    let v = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(v).unwrap_or_default())
}

fn lenient_bool<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(serde_json::Value::deserialize(d)?.as_bool().unwrap_or(false))
}

/// What [`free_deck`] needs to know of a deck.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeckUse {
    pub loaded: bool,
    pub playing: bool,
}

/// Crossfader position past which deck B is the one the room hears (0 = all A, 1 = all B).
const CROSSFADER_CENTER: f32 = 0.5;

/// The deck a song picked while DJ mode is on goes to ([`LoadTarget::FreeDeck`]): an empty deck
/// first, then one that is not playing (deck A before deck B); with both playing, the one the
/// crossfader is turned away from. Returns the deck's index (0 = A).
pub fn free_deck(decks: [DeckUse; super::DECK_COUNT], crossfader: f32) -> usize {
    if let Some(i) = decks.iter().position(|d| !d.loaded) {
        return i;
    }
    if let Some(i) = decks.iter().position(|d| !d.playing) {
        return i;
    }
    if crossfader >= CROSSFADER_CENTER {
        0
    } else {
        1
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DjSettings {
    pub latency: DjLatency,
    /// Tempo fader range, +/- percent, 4..=50.
    pub tempo_range_pct: f32,
    /// Pitch bend strength (percent of speed), 1..=10.
    pub bend_pct: f32,
    /// Cue points, loops and hot cues snap to the beat grid.
    pub quantize: bool,
    pub crossfader_curve: CrossfaderCurve,
    /// Soft limiter on the master output.
    pub master_limiter: bool,
    /// Each song's level is matched to a common one on load.
    pub auto_gain: bool,
    /// Largest boost of the channel EQ bands (dB), 0..=12. Kill is always available.
    pub eq_max_boost_db: f32,
    pub sampler_volume: f32,
    /// A song loaded while the other deck plays takes its tempo.
    pub sync_on_load: bool,
    /// Length of an automatic transition (beats): 8, 16 or 32.
    pub auto_mix_beats: u32,
    pub load_target: LoadTarget,
    /// RAM the decoded decks may use (MB), 128..=4096.
    pub memory_mb: u32,
    /// Keyboard shortcuts on (they are optional extras; everything works with the mouse).
    pub keys_enabled: bool,
    /// Rebinds: action id to key (an empty key unbinds). See [`super::keymap`].
    pub key_overrides: BTreeMap<String, String>,
    /// Colored waveform (low / mid / high) instead of one color.
    pub waveform_colors: bool,
    /// Beats shown by the zoomed waveform, 2..=32.
    pub zoom_beats: u8,
    pub show_beat_grid: bool,
    /// An automatic transition swaps the bass halfway (the outgoing song's low end out, the
    /// incoming one's in); off, it only moves the crossfader.
    pub bass_swap: bool,
    /// The beginner screen: hides hot cues, loops, trim, the tempo fader and pitch bend (Sync does
    /// the matching). "Show all controls" turns it off.
    pub simple_view: bool,
    /// The one-time "Your first mix" card.
    pub show_tips: bool,
    /// DJ mode has its own entry in the app's menu (the desktop's sidebar, Android's navigation).
    pub show_entry: bool,
    /// Order and visibility of the screen's sections and the sound of each sampler pad.
    pub layout: DjLayout,
    /// Homifito DJ: the mascot at his booth.
    pub homifito: HomifitoDj,
}

impl Default for DjSettings {
    fn default() -> Self {
        Self {
            latency: DjLatency::default(),
            tempo_range_pct: DEFAULT_TEMPO_RANGE_PCT,
            bend_pct: DEFAULT_BEND_PCT,
            quantize: true,
            crossfader_curve: CrossfaderCurve::default(),
            master_limiter: true,
            auto_gain: true,
            eq_max_boost_db: DEFAULT_EQ_BOOST_DB,
            sampler_volume: DEFAULT_SAMPLER_VOLUME,
            sync_on_load: true,
            auto_mix_beats: DEFAULT_AUTO_MIX_BEATS,
            load_target: LoadTarget::default(),
            memory_mb: DEFAULT_MEMORY_MB,
            keys_enabled: true,
            key_overrides: BTreeMap::new(),
            waveform_colors: true,
            zoom_beats: DEFAULT_ZOOM_BEATS,
            show_beat_grid: true,
            bass_swap: true,
            simple_view: true,
            show_tips: true,
            show_entry: true,
            layout: DjLayout::default(),
            homifito: HomifitoDj::default(),
        }
    }
}

/// The allowed transition length closest to `beats`.
pub fn nearest_auto_mix_beats(beats: u32) -> u32 {
    AUTO_MIX_BEATS.into_iter().min_by_key(|b| b.abs_diff(beats)).unwrap_or(DEFAULT_AUTO_MIX_BEATS)
}

fn clamp_or(v: f32, min: f32, max: f32, default: f32) -> f32 {
    if v.is_finite() {
        v.clamp(min, max)
    } else {
        default
    }
}

impl DjSettings {
    /// Every value inside its range (NaN becomes the default).
    pub fn clamped(&self) -> Self {
        Self {
            tempo_range_pct: clamp_or(
                self.tempo_range_pct,
                MIN_TEMPO_RANGE_PCT,
                MAX_TEMPO_RANGE_PCT,
                DEFAULT_TEMPO_RANGE_PCT,
            ),
            bend_pct: clamp_or(self.bend_pct, MIN_BEND_PCT, MAX_BEND_PCT, DEFAULT_BEND_PCT),
            eq_max_boost_db: clamp_or(self.eq_max_boost_db, MIN_EQ_BOOST_DB, MAX_EQ_BOOST_DB, DEFAULT_EQ_BOOST_DB),
            sampler_volume: clamp_or(self.sampler_volume, 0.0, 1.0, DEFAULT_SAMPLER_VOLUME),
            auto_mix_beats: nearest_auto_mix_beats(self.auto_mix_beats),
            memory_mb: self.memory_mb.clamp(MIN_MEMORY_MB, MAX_MEMORY_MB),
            zoom_beats: self.zoom_beats.clamp(MIN_ZOOM_BEATS, MAX_ZOOM_BEATS),
            key_overrides: self.key_overrides.clone(),
            layout: self.layout.clamped(),
            ..self.clone()
        }
    }

    /// The part of the settings the real-time mixer reads (small and `Copy`, so it can travel
    /// through the command ring).
    pub fn mixer(&self) -> MixerConfig {
        let s = self.clamped();
        MixerConfig {
            tempo_range_pct: s.tempo_range_pct,
            bend_pct: s.bend_pct,
            quantize: s.quantize,
            crossfader_curve: s.crossfader_curve,
            master_limiter: s.master_limiter,
            auto_gain: s.auto_gain,
            eq_max_boost_db: s.eq_max_boost_db,
            sampler_volume: s.sampler_volume,
            sync_on_load: s.sync_on_load,
            auto_mix_beats: s.auto_mix_beats,
            bass_swap: s.bass_swap,
        }
    }
}

/// What the audio callback needs of [`DjSettings`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MixerConfig {
    pub tempo_range_pct: f32,
    pub bend_pct: f32,
    pub quantize: bool,
    pub crossfader_curve: CrossfaderCurve,
    pub master_limiter: bool,
    pub auto_gain: bool,
    pub eq_max_boost_db: f32,
    pub sampler_volume: f32,
    pub sync_on_load: bool,
    pub auto_mix_beats: u32,
    pub bass_swap: bool,
}

impl Default for MixerConfig {
    fn default() -> Self {
        DjSettings::default().mixer()
    }
}

/// [`DjSettings`] falling back to the defaults when unreadable.
pub fn lenient_dj<'de, D: Deserializer<'de>>(d: D) -> Result<DjSettings, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Any {
        Known(DjSettings),
        Other(serde::de::IgnoredAny),
    }
    Ok(match Any::deserialize(d)? {
        Any::Known(s) => s.clamped(),
        Any::Other(_) => DjSettings::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_follow_the_plan() {
        let s = DjSettings::default();
        assert_eq!(s.auto_mix_beats, 16);
        assert!(s.sync_on_load && s.quantize && s.auto_gain && s.master_limiter);
        // beginner first: the simple screen, the first-mix card, the bass swap, an entry in the menu
        assert!(s.simple_view && s.show_tips && s.bass_swap && s.show_entry);
        assert!(s.mixer().bass_swap);
        assert_eq!(s.tempo_range_pct, 8.0);
        assert_eq!(s.memory_mb, 1024);
        assert_eq!(s.load_target, LoadTarget::FreeDeck);
        assert_eq!(s.clamped(), s, "defaults are already in range");
        assert_eq!(DjLatency::Low.frames(), 256);
        assert_eq!(DjLatency::Safe.frames(), 1024);
    }

    #[test]
    fn homifito_defaults_and_lenient_values() {
        let d = HomifitoDj::default();
        assert_eq!((d.mode, d.place, d.look), (HomifitoMode::Booth, HomifitoPlace::Top, false));
        let s: DjSettings = serde_json::from_str(r#"{}"#).unwrap();
        assert_eq!(s.homifito, d, "old files have no homifito");
        let s: DjSettings =
            serde_json::from_str(r#"{"homifito":{"mode":"big","place":"bottom","look":true}}"#).unwrap();
        assert_eq!(s.homifito, HomifitoDj { mode: HomifitoMode::Big, place: HomifitoPlace::Bottom, look: true });
        let s: DjSettings = serde_json::from_str(r#"{"homifito":{"mode":"huge","place":7,"look":"yes"}}"#).unwrap();
        assert_eq!(s.homifito, d);
        let s: DjSettings = serde_json::from_str(r#"{"homifito":{"mode":"off"}}"#).unwrap();
        assert_eq!(s.homifito.mode, HomifitoMode::Off);
        assert_eq!(s.clamped().homifito.mode, HomifitoMode::Off, "clamping keeps it");
    }

    #[test]
    fn clamping_pulls_everything_into_range() {
        let wild = DjSettings {
            tempo_range_pct: 900.0,
            bend_pct: -3.0,
            eq_max_boost_db: f32::NAN,
            sampler_volume: 7.0,
            auto_mix_beats: 13,
            memory_mb: 1,
            zoom_beats: 200,
            ..DjSettings::default()
        };
        let c = wild.clamped();
        assert_eq!(c.tempo_range_pct, MAX_TEMPO_RANGE_PCT);
        assert_eq!(c.bend_pct, MIN_BEND_PCT);
        assert_eq!(c.eq_max_boost_db, DEFAULT_EQ_BOOST_DB);
        assert_eq!(c.sampler_volume, 1.0);
        assert_eq!(c.auto_mix_beats, 16);
        assert_eq!(c.memory_mb, MIN_MEMORY_MB);
        assert_eq!(c.zoom_beats, MAX_ZOOM_BEATS);
        assert_eq!(nearest_auto_mix_beats(0), 8);
        assert_eq!(nearest_auto_mix_beats(100), 32);
    }

    #[test]
    fn bad_json_falls_back_to_the_defaults() {
        #[derive(Deserialize)]
        struct Holder {
            #[serde(default, deserialize_with = "lenient_dj")]
            dj: DjSettings,
        }
        for json in [r#"{"dj": 5}"#, r#"{"dj": "x"}"#, r#"{"dj": null}"#, r#"{}"#] {
            let h: Holder = serde_json::from_str(json).unwrap();
            assert_eq!(h.dj, DjSettings::default(), "{json}");
        }
        // a partial object keeps what is valid and clamps it
        let h: Holder = serde_json::from_str(r#"{"dj": {"tempo_range_pct": 99, "latency": "low"}}"#).unwrap();
        assert_eq!(h.dj.tempo_range_pct, MAX_TEMPO_RANGE_PCT);
        assert_eq!(h.dj.latency, DjLatency::Low);
        // an unknown enum value becomes the default variant, unknown keys are ignored
        let s: DjSettings =
            serde_json::from_str(r#"{"latency": "turbo", "load_target": "x", "crossfader_curve": 3, "new": 1}"#)
                .unwrap_or_default();
        assert_eq!(s.latency, DjLatency::Balanced);
    }

    #[test]
    fn songs_go_to_the_free_deck() {
        let empty = DeckUse::default();
        let idle = DeckUse { loaded: true, playing: false };
        let live = DeckUse { loaded: true, playing: true };
        assert_eq!(free_deck([empty, empty], 0.5), 0, "deck A when both are empty");
        assert_eq!(free_deck([live, empty], 0.0), 1);
        assert_eq!(free_deck([idle, empty], 0.0), 1, "an empty deck before a stopped one");
        assert_eq!(free_deck([live, idle], 0.0), 1);
        assert_eq!(free_deck([idle, live], 1.0), 0);
        assert_eq!(free_deck([idle, idle], 0.5), 0);
        // both playing: the one the room hears less
        assert_eq!(free_deck([live, live], 0.9), 0);
        assert_eq!(free_deck([live, live], 0.1), 1);
    }

    #[test]
    fn round_trips() {
        let s = DjSettings { quantize: false, auto_mix_beats: 32, ..DjSettings::default() };
        let back: DjSettings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }
}
