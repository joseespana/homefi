//! Exercise mode options. The same JSON on desktop (`desktop.json`) and Android; lenient like the
//! other settings: an unreadable value falls back to the defaults instead of failing to load.

use serde::{Deserialize, Deserializer, Serialize};

use super::model::{round_minutes, PresetId, MAX_RIDE_MINUTES, MIN_RIDE_MINUTES};

pub const DEFAULT_RIDE_MINUTES: u16 = 30;
pub const MIN_TOLERANCE_RPM: u8 = 2;
pub const MAX_TOLERANCE_RPM: u8 = 12;
pub const DEFAULT_TOLERANCE_RPM: u8 = 4;
pub const MAX_ANALYSIS_BUDGET: u8 = 50;
pub const DEFAULT_ANALYSIS_BUDGET: u8 = 12;
pub const MAX_COUNTDOWN_SECONDS: u8 = 10;
pub const DEFAULT_COUNTDOWN_SECONDS: u8 = 3;
pub const MIN_NOTICE_SECONDS: u8 = 5;
pub const MAX_NOTICE_SECONDS: u8 = 30;
pub const DEFAULT_NOTICE_SECONDS: u8 = 10;
pub const DEFAULT_CUE_VOLUME: f32 = 0.8;
pub const MAX_PULSE_OFFSET_MS: i16 = 500;
pub const MIN_TV_PORT: u16 = 1024;
pub const DEFAULT_TV_PORT: u16 = 47801;
pub const MAX_TEMPO_NUDGE_PCT: u8 = 8;
/// Auto-saved rides kept (the rider's own kept rides and favorites don't count), 5..=200.
pub const MIN_RIDE_HISTORY: u16 = 5;
pub const MAX_RIDE_HISTORY: u16 = 200;
pub const DEFAULT_RIDE_HISTORY: u16 = 30;
/// `tv_window_display` value meaning "a second screen when there is one".
pub const TV_DISPLAY_AUTO: &str = "auto";
/// Longest display name kept (a screen's name from the OS).
pub const MAX_DISPLAY_NAME_CHARS: usize = 80;

/// Where the songs of a ride come from first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SourcesMode {
    TidalFirst,
    LibraryOnly,
    #[default]
    #[serde(other)]
    LibraryFirst,
}

/// How the beat pulse is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PulseStyle {
    Bar,
    Flash,
    #[default]
    #[serde(other)]
    Ring,
}

/// How the course mountain is drawn (docs/MOUNTAIN.md): the three presets of one renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CourseLook {
    /// Calm slate ground, snow on the high ground, lit rim.
    Alpine,
    /// A dark stage with a strong wash under the ridge.
    Stage,
    /// Warm summits over a cool foot, the zone as a thin line (the default).
    #[default]
    #[serde(other)]
    Ribbon,
}

/// What the TV does when the rider uses one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TvAudio {
    /// The TV plays the music too: the ride's audio is sent to it (a listen-only guest of a ride hub).
    Tv,
    /// Pictures only: the music stays on the phone or computer (headphones). The default: José
    /// rides with the TV in front and headphones on.
    #[default]
    #[serde(other)]
    Phone,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExerciseSettings {
    /// Exercise has its own entry in the app's menu (the desktop's sidebar, Android's navigation).
    pub show_entry: bool,
    /// Caps for new riders: cadence 60..110, standing never under 60, short sprints with twice the rest.
    pub beginner_mode: bool,
    pub default_preset: PresetId,
    /// Default ride length, 10..=90 in steps of 5.
    pub ride_minutes: u16,
    /// How far a song's tempo may be from the cadence (RPM), 2..=12.
    pub cadence_tolerance_rpm: u8,
    /// A song of 120..180 BPM counts one pedal stroke every two beats.
    pub allow_half_time: bool,
    /// A very slow song counts two pedal strokes a beat.
    pub allow_double_time: bool,
    pub sources: SourcesMode,
    /// Songs the builder may analyze (decode) per ride when no tag or Tidal tempo is known, 0..=50.
    pub analysis_budget: u8,
    pub allow_explicit: bool,
    /// Seconds of 3-2-1 before a change, 0..=10.
    pub countdown_seconds: u8,
    /// "Next: standing climb" shows this long before the change (s), 5..=30.
    pub segment_notice_seconds: u8,
    pub audible_cues: bool,
    /// 0..=1.
    pub cue_volume: f32,
    /// A click on every pedal stroke.
    pub metronome_click: bool,
    pub pulse_style: PulseStyle,
    /// Moves the pulse to match a Bluetooth speaker's delay (ms), -500..=500.
    pub pulse_offset_ms: i16,
    pub keep_screen_on: bool,
    pub tv_enabled: bool,
    /// Tv (the TV plays the music too) or Phone (pictures only).
    pub tv_audio: TvAudio,
    /// 1024..=65535.
    pub tv_port: u16,
    /// Desktop: the TV window opens fullscreen.
    pub tv_window_fullscreen: bool,
    /// Desktop: the screen the TV window opens on, as the OS names it; [`TV_DISPLAY_AUTO`] = a second screen when
    /// there is one, else the main one.
    pub tv_window_display: String,
    /// The TV shows the song's lyrics, line by line, when the app has them.
    pub tv_lyrics: bool,
    /// How the course mountain looks (setup, preview, ride, summary and the TV).
    pub course_look: CourseLook,
    /// The course marks its tallest summits with their time.
    pub course_summits: bool,
    /// The ride screen stays dark whatever the app's theme.
    pub ride_always_dark: bool,
    /// Android only: a short buzz on every change of part.
    pub haptic_cues: bool,
    /// The "not medical advice" notice was accepted.
    pub health_notice_seen: bool,
    /// Phase 2: speed songs up or down a few percent to the cadence, 0..=8 (0 = off).
    pub tempo_nudge_pct: u8,
    /// Every built ride is saved automatically; this many such rides are kept, oldest pruned first
    /// (kept rides and favorites never), 5..=200.
    pub ride_history_max: u16,
}

impl Default for ExerciseSettings {
    fn default() -> Self {
        Self {
            show_entry: true,
            beginner_mode: true,
            default_preset: PresetId::FirstRide,
            ride_minutes: DEFAULT_RIDE_MINUTES,
            cadence_tolerance_rpm: DEFAULT_TOLERANCE_RPM,
            allow_half_time: true,
            allow_double_time: false,
            sources: SourcesMode::LibraryFirst,
            analysis_budget: DEFAULT_ANALYSIS_BUDGET,
            allow_explicit: true,
            countdown_seconds: DEFAULT_COUNTDOWN_SECONDS,
            segment_notice_seconds: DEFAULT_NOTICE_SECONDS,
            audible_cues: true,
            cue_volume: DEFAULT_CUE_VOLUME,
            metronome_click: false,
            pulse_style: PulseStyle::Ring,
            pulse_offset_ms: 0,
            keep_screen_on: true,
            tv_enabled: false,
            tv_audio: TvAudio::Phone,
            tv_port: DEFAULT_TV_PORT,
            tv_window_fullscreen: true,
            tv_window_display: TV_DISPLAY_AUTO.into(),
            tv_lyrics: true,
            course_look: CourseLook::Ribbon,
            course_summits: false,
            ride_always_dark: true,
            haptic_cues: true,
            health_notice_seen: false,
            tempo_nudge_pct: 0,
            ride_history_max: DEFAULT_RIDE_HISTORY,
        }
    }
}

impl ExerciseSettings {
    /// Every value inside its range (NaN becomes the default).
    pub fn clamped(&self) -> Self {
        Self {
            ride_minutes: round_minutes(self.ride_minutes.clamp(MIN_RIDE_MINUTES, MAX_RIDE_MINUTES)),
            cadence_tolerance_rpm: self.cadence_tolerance_rpm.clamp(MIN_TOLERANCE_RPM, MAX_TOLERANCE_RPM),
            analysis_budget: self.analysis_budget.min(MAX_ANALYSIS_BUDGET),
            countdown_seconds: self.countdown_seconds.min(MAX_COUNTDOWN_SECONDS),
            segment_notice_seconds: self.segment_notice_seconds.clamp(MIN_NOTICE_SECONDS, MAX_NOTICE_SECONDS),
            cue_volume: if self.cue_volume.is_finite() { self.cue_volume.clamp(0.0, 1.0) } else { DEFAULT_CUE_VOLUME },
            pulse_offset_ms: self.pulse_offset_ms.clamp(-MAX_PULSE_OFFSET_MS, MAX_PULSE_OFFSET_MS),
            tv_port: self.tv_port.max(MIN_TV_PORT),
            tempo_nudge_pct: self.tempo_nudge_pct.min(MAX_TEMPO_NUDGE_PCT),
            ride_history_max: self.ride_history_max.clamp(MIN_RIDE_HISTORY, MAX_RIDE_HISTORY),
            tv_window_display: match self.tv_window_display.trim() {
                "" => TV_DISPLAY_AUTO.into(),
                name => name.chars().take(MAX_DISPLAY_NAME_CHARS).collect(),
            },
            ..self.clone()
        }
    }
}

/// [`ExerciseSettings`] falling back to the defaults when unreadable.
pub fn lenient_exercise<'de, D: Deserializer<'de>>(d: D) -> Result<ExerciseSettings, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Any {
        Known(ExerciseSettings),
        Other(serde::de::IgnoredAny),
    }
    Ok(match Any::deserialize(d)? {
        Any::Known(s) => s.clamped(),
        Any::Other(_) => ExerciseSettings::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_follow_the_plan() {
        let s = ExerciseSettings::default();
        assert!(s.show_entry && s.beginner_mode && s.allow_half_time && s.allow_explicit);
        assert!(!s.allow_double_time && !s.metronome_click && !s.tv_enabled && !s.health_notice_seen);
        assert_eq!(s.tv_audio, TvAudio::Phone);
        assert_eq!((s.ride_minutes, s.cadence_tolerance_rpm, s.analysis_budget), (30, 4, 12));
        assert_eq!((s.countdown_seconds, s.segment_notice_seconds, s.tv_port), (3, 10, 47801));
        assert_eq!((s.cue_volume, s.pulse_offset_ms, s.tempo_nudge_pct), (0.8, 0, 0));
        assert_eq!(
            (s.default_preset, s.sources, s.pulse_style),
            (PresetId::FirstRide, SourcesMode::LibraryFirst, PulseStyle::Ring)
        );
        assert!(s.audible_cues && s.keep_screen_on && s.tv_window_fullscreen);
        assert!(s.ride_always_dark && s.haptic_cues);
        assert_eq!(s.tv_window_display, "auto");
        assert!(s.tv_lyrics, "lyrics on the TV are on by default");
        assert_eq!((s.course_look, s.course_summits), (CourseLook::Ribbon, false));
        assert_eq!(s.ride_history_max, 30);
        assert_eq!(s.clamped(), s, "defaults are already in range");
    }

    #[test]
    fn clamping_pulls_everything_into_range() {
        let wild = ExerciseSettings {
            ride_minutes: 400,
            cadence_tolerance_rpm: 0,
            analysis_budget: 200,
            countdown_seconds: 99,
            segment_notice_seconds: 1,
            cue_volume: f32::NAN,
            pulse_offset_ms: -9000,
            tv_port: 80,
            tempo_nudge_pct: 50,
            ride_history_max: 0,
            tv_window_display: "  ".into(),
            ..ExerciseSettings::default()
        };
        let c = wild.clamped();
        assert_eq!((c.ride_minutes, c.cadence_tolerance_rpm, c.analysis_budget), (90, 2, 50));
        assert_eq!((c.countdown_seconds, c.segment_notice_seconds, c.cue_volume), (10, 5, 0.8));
        assert_eq!((c.pulse_offset_ms, c.tv_port, c.tempo_nudge_pct), (-500, 1024, 8));
        assert_eq!(c.tv_window_display, TV_DISPLAY_AUTO, "a blank name means auto");
        assert_eq!(c.ride_history_max, MIN_RIDE_HISTORY);
        assert_eq!(ExerciseSettings { ride_history_max: 9999, ..Default::default() }.clamped().ride_history_max, 200);
        let long = ExerciseSettings { tv_window_display: "x".repeat(500), ..Default::default() }.clamped();
        assert_eq!(long.tv_window_display.chars().count(), MAX_DISPLAY_NAME_CHARS);
        assert_eq!(ExerciseSettings { ride_minutes: 2, ..Default::default() }.clamped().ride_minutes, 10);
        assert_eq!(ExerciseSettings { ride_minutes: 47, ..Default::default() }.clamped().ride_minutes, 45);
    }

    #[test]
    fn json_is_lenient() {
        #[derive(Deserialize)]
        struct Holder {
            #[serde(default, deserialize_with = "lenient_exercise")]
            exercise: ExerciseSettings,
        }
        let h: Holder = serde_json::from_str(r#"{"exercise": "garbage"}"#).unwrap();
        assert_eq!(h.exercise, ExerciseSettings::default());
        let h: Holder =
            serde_json::from_str(r#"{"exercise": {"ride_minutes": 3, "sources": "mars", "unknown": 1}}"#).unwrap();
        assert_eq!((h.exercise.ride_minutes, h.exercise.sources), (10, SourcesMode::LibraryFirst));
        let h: Holder = serde_json::from_str("{}").unwrap();
        assert_eq!(h.exercise, ExerciseSettings::default());
        let h: Holder =
            serde_json::from_str(r#"{"exercise": {"course_look": "alpine", "course_summits": true}}"#).unwrap();
        assert_eq!((h.exercise.course_look, h.exercise.course_summits), (CourseLook::Alpine, true));
        let h: Holder = serde_json::from_str(r#"{"exercise": {"course_look": "neon"}}"#).unwrap();
        assert_eq!(h.exercise.course_look, CourseLook::Ribbon, "an unknown look falls back to ribbon");
    }
}
