//! Exercise mode (indoor cycling) for Android: building a ride, the ride clock, the cue sounds, the settings,
//! the AI chat and the TV (server, discovery and the client's parsing).
//!
//! Everything is `homefi_core::exercise`; this file only converts and forwards, so the rules, the defaults
//! and the sounds are the same as on desktop. The player, the screens and the cue playback are Kotlin.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use homefi_core::dj::{DjAnalysis as CoreAnalysis, Grid};
use homefi_core::exercise as ex;
use homefi_core::humane::Lang;
use homefi_core::playlist as pl;

use crate::ai::{core_config, AiConfig};
use crate::dj::DjGrid;
use crate::playlists::{count, library_index, HomePlaylist, LibraryRef, PlaylistEntry};
use crate::share::SharePermissions;
use crate::{HomefiEngine, HomefiError, Result};

/// Cache folder of the analyses, inside the app dir (the DJ's).
const ANALYSIS_DIR: [&str; 2] = ["dj", "analysis"];

/// Same variants on both sides: two `From` impls.
macro_rules! mirror_enum {
    ($ffi:ident <=> $core:ty { $($variant:ident),+ $(,)? }) => {
        impl From<$core> for $ffi {
            fn from(value: $core) -> Self {
                match value { $(<$core>::$variant => Self::$variant),+ }
            }
        }
        impl From<$ffi> for $core {
            fn from(value: $ffi) -> Self {
                match value { $($ffi::$variant => <$core>::$variant),+ }
            }
        }
    };
}

// ------------------------------------------------------------------- enums

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SegmentKind {
    WarmUp,
    SeatedFlat,
    SeatedClimb,
    StandingClimb,
    Jumps,
    Sprint,
    Recovery,
    CoolDown,
}
mirror_enum!(SegmentKind <=> ex::SegmentKind {
    WarmUp, SeatedFlat, SeatedClimb, StandingClimb, Jumps, Sprint, Recovery, CoolDown
});

/// The ride styles; each works at any length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PresetId {
    FirstRide,
    Easy,
    Classic,
    Intervals,
}
mirror_enum!(PresetId <=> ex::PresetId { FirstRide, Easy, Classic, Intervals });

/// How famous the songs of a ride should be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Popularity {
    /// The genre's big hits by its famous acts (the default).
    Hits,
    Mix,
    Discover,
}
mirror_enum!(Popularity <=> ex::Popularity { Hits, Mix, Discover });

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SourcesMode {
    LibraryFirst,
    TidalFirst,
    LibraryOnly,
}
mirror_enum!(SourcesMode <=> ex::SourcesMode { LibraryFirst, TidalFirst, LibraryOnly });

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PulseStyle {
    Ring,
    Bar,
    Flash,
}
mirror_enum!(PulseStyle <=> ex::PulseStyle { Ring, Bar, Flash });

/// How the course mountain is drawn (docs/MOUNTAIN.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum CourseLook {
    Ribbon,
    Alpine,
    Stage,
}
mirror_enum!(CourseLook <=> ex::CourseLook { Ribbon, Alpine, Stage });

/// How the rider is drawn on the course mountain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum CourseRider {
    Dot,
    Bike,
}
mirror_enum!(CourseRider <=> ex::CourseRider { Dot, Bike });

/// What the rider's bike console reads out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ConsoleUnit {
    Rpm,
    Kph,
    Mph,
    /// No console, or not looked at: targets show in RPM.
    None,
}
mirror_enum!(ConsoleUnit <=> ex::ConsoleUnit { Rpm, Kph, Mph, None });

/// The unit a target is shown in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TargetUnit {
    Kph,
    Mph,
    Rpm,
}
mirror_enum!(TargetUnit <=> ex::TargetUnit { Kph, Mph, Rpm });

/// Tv: the TV plays the music too. Phone: pictures only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TvAudio {
    Tv,
    Phone,
}
mirror_enum!(TvAudio <=> ex::TvAudio { Tv, Phone });

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BpmSource {
    Tag,
    Tidal,
    Analysis,
}
mirror_enum!(BpmSource <=> ex::BpmSource { Tag, Tidal, Analysis });

/// Pedal strokes per beat: one (`Whole`), one every two beats (`Half`) or two (`Double`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BeatRatio {
    Whole,
    Half,
    Double,
}
mirror_enum!(BeatRatio <=> ex::BeatRatio { Whole, Half, Double });

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PulseSource {
    SongBeat,
    Metronome,
}
mirror_enum!(PulseSource <=> ex::PulseSource { SongBeat, Metronome });

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum IntervalPhase {
    Work,
    Rest,
}
mirror_enum!(IntervalPhase <=> ex::IntervalPhase { Work, Rest });

/// The ride's sounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum CueKind {
    Countdown3,
    Countdown2,
    Countdown1,
    Change,
    Standing,
    Seated,
    SprintGo,
    Recover,
    Finish,
    Click,
    Resistance,
}
mirror_enum!(CueKind <=> ex::CueKind {
    Countdown3, Countdown2, Countdown1, Change, Standing, Seated, SprintGo, Recover, Finish, Click, Resistance
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum RidePosition {
    Seated,
    Standing,
    /// Up and down every `beats` beats.
    Alternating {
        beats: u8,
    },
}

impl From<ex::Position> for RidePosition {
    fn from(p: ex::Position) -> Self {
        match p {
            ex::Position::Seated => Self::Seated,
            ex::Position::Standing => Self::Standing,
            ex::Position::Alternating { beats } => Self::Alternating { beats },
        }
    }
}

impl From<RidePosition> for ex::Position {
    fn from(p: RidePosition) -> Self {
        match p {
            RidePosition::Seated => Self::Seated,
            RidePosition::Standing => Self::Standing,
            RidePosition::Alternating { beats } => Self::Alternating { beats },
        }
    }
}

// -------------------------------------------------------------------- bike

/// The rider's bike: what its console shows and how many resistance levels its knob has.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct BikeProfile {
    pub console: ConsoleUnit,
    /// Knob positions, 0..=32; 0 = unknown (no level is shown).
    pub resistance_levels: u8,
    /// km/h the console shows per RPM; 0 = not calibrated (targets show in RPM).
    pub kph_per_rpm: f32,
}

impl From<ex::BikeProfile> for BikeProfile {
    fn from(b: ex::BikeProfile) -> Self {
        Self { console: b.console.into(), resistance_levels: b.resistance_levels, kph_per_rpm: b.kph_per_rpm }
    }
}

impl From<BikeProfile> for ex::BikeProfile {
    fn from(b: BikeProfile) -> Self {
        Self { console: b.console.into(), resistance_levels: b.resistance_levels, kph_per_rpm: b.kph_per_rpm }
    }
}

/// A target ready to show: `value` with `min`..`max` in `unit`, and the cadence it came from.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct ShownTarget {
    pub unit: TargetUnit,
    /// The big number: the center of the range.
    pub value: f32,
    pub min: f32,
    pub max: f32,
    /// The center cadence (RPM): "pedal at about 75 RPM" stays true whatever the unit.
    pub rpm: u16,
}

impl From<ex::ShownTarget> for ShownTarget {
    fn from(t: ex::ShownTarget) -> Self {
        Self { unit: t.unit.into(), value: t.value, min: t.min, max: t.max, rpm: t.rpm }
    }
}

/// What the calibration asks of the rider: pedal at `rpm` for `seconds` (clicks keep the pace), then type
/// the speed the console shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct CalibrationSpec {
    pub rpm: u16,
    pub seconds: u8,
}

#[uniffi::export]
pub fn exercise_calibration() -> CalibrationSpec {
    CalibrationSpec { rpm: ex::bike::CALIBRATION_RPM, seconds: ex::bike::CALIBRATION_SECONDS }
}

/// The target of a cadence range in the bike's unit (km/h or mph once calibrated, else RPM).
#[uniffi::export]
pub fn exercise_shown_target(bike: BikeProfile, min_rpm: u16, max_rpm: u16) -> ShownTarget {
    ex::BikeProfile::from(bike).target(ex::RpmRange { min_rpm, max_rpm }).into()
}

/// The knob position (1..=levels) for an effort 1..=10; `None` when the bike's levels are unknown.
#[uniffi::export]
pub fn exercise_bike_level(bike: BikeProfile, effort: u8) -> Option<u8> {
    ex::BikeProfile::from(bike).level(effort)
}

/// km/h per RPM from one reading (`speed` in the console's unit while pedaling `rpm`); `None` for a console
/// without speed or a reading nobody's bike gives.
#[uniffi::export]
pub fn exercise_calibrate(console: ConsoleUnit, rpm: u16, speed: f32) -> Option<f32> {
    ex::BikeProfile::calibrate(console.into(), rpm, speed)
}

// ---------------------------------------------------------------- settings

/// Every Exercise option (defaults and ranges are the core's).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ExerciseSettings {
    pub show_entry: bool,
    pub beginner_mode: bool,
    pub default_preset: PresetId,
    /// 10..=90 in steps of 5.
    pub ride_minutes: u16,
    /// 2..=12.
    pub cadence_tolerance_rpm: u8,
    pub allow_half_time: bool,
    pub allow_double_time: bool,
    pub sources: SourcesMode,
    /// 0..=50 songs analyzed per ride.
    pub analysis_budget: u8,
    pub allow_explicit: bool,
    /// 0..=10.
    pub countdown_seconds: u8,
    /// 5..=30.
    pub segment_notice_seconds: u8,
    pub audible_cues: bool,
    /// 0..=1.
    pub cue_volume: f32,
    pub metronome_click: bool,
    pub pulse_style: PulseStyle,
    /// -500..=500.
    pub pulse_offset_ms: i16,
    pub keep_screen_on: bool,
    pub tv_enabled: bool,
    pub tv_audio: TvAudio,
    /// 1024..=65535.
    pub tv_port: u16,
    pub tv_window_fullscreen: bool,
    /// Desktop: the screen the TV window opens on ("auto" = a second screen when there is one).
    pub tv_window_display: String,
    /// The TV shows the song's lyrics.
    #[uniffi(default = true)]
    pub tv_lyrics: bool,
    /// How the course mountain looks.
    pub course_look: CourseLook,
    /// Label the tallest summits with their time.
    #[uniffi(default = false)]
    pub course_summits: bool,
    pub ride_always_dark: bool,
    /// Android only: a short buzz on every change of part.
    pub haptic_cues: bool,
    pub health_notice_seen: bool,
    /// 0..=8, 0 = off (phase 2).
    pub tempo_nudge_pct: u8,
    /// Automatic rides kept, 5..=200 (kept rides and favorites never count).
    #[uniffi(default = 30)]
    pub ride_history_max: u16,
    /// The setup's taste answers, remembered for the next ride.
    pub taste: RideTaste,
    /// The rider's bike (console unit, knob levels, speed calibration).
    pub bike: BikeProfile,
    /// Resistance steps on climbs, 0..=4 (0 = off).
    #[uniffi(default = 3)]
    pub climb_steps: u8,
    /// Seconds the next song fades in when a part ends, 0..=8 (0 = the cut).
    #[uniffi(default = 3.0)]
    pub crossfade_seconds: f32,
    /// How the rider is drawn on the mountain.
    pub course_rider: CourseRider,
    /// Android only: a large Pause button and a column of controls instead of the small icon row.
    #[uniffi(default = false)]
    pub ride_big_controls: bool,
    /// A tap on the ride's song offers another one that fits the part.
    #[uniffi(default = true)]
    pub ride_swap: bool,
    /// When nothing already found fits, the AI proposes the replacement (uses the rider's key).
    #[uniffi(default = true)]
    pub swap_uses_ai: bool,
}

impl From<ex::ExerciseSettings> for ExerciseSettings {
    fn from(s: ex::ExerciseSettings) -> Self {
        Self {
            show_entry: s.show_entry,
            beginner_mode: s.beginner_mode,
            default_preset: s.default_preset.into(),
            ride_minutes: s.ride_minutes,
            cadence_tolerance_rpm: s.cadence_tolerance_rpm,
            allow_half_time: s.allow_half_time,
            allow_double_time: s.allow_double_time,
            sources: s.sources.into(),
            analysis_budget: s.analysis_budget,
            allow_explicit: s.allow_explicit,
            countdown_seconds: s.countdown_seconds,
            segment_notice_seconds: s.segment_notice_seconds,
            audible_cues: s.audible_cues,
            cue_volume: s.cue_volume,
            metronome_click: s.metronome_click,
            pulse_style: s.pulse_style.into(),
            pulse_offset_ms: s.pulse_offset_ms,
            keep_screen_on: s.keep_screen_on,
            tv_enabled: s.tv_enabled,
            tv_audio: s.tv_audio.into(),
            tv_port: s.tv_port,
            tv_window_fullscreen: s.tv_window_fullscreen,
            tv_window_display: s.tv_window_display,
            tv_lyrics: s.tv_lyrics,
            course_look: s.course_look.into(),
            course_summits: s.course_summits,
            ride_always_dark: s.ride_always_dark,
            haptic_cues: s.haptic_cues,
            health_notice_seen: s.health_notice_seen,
            tempo_nudge_pct: s.tempo_nudge_pct,
            ride_history_max: s.ride_history_max,
            taste: s.taste.into(),
            bike: s.bike.into(),
            climb_steps: s.climb_steps,
            crossfade_seconds: s.crossfade_seconds,
            course_rider: s.course_rider.into(),
            ride_big_controls: s.ride_big_controls,
            ride_swap: s.ride_swap,
            swap_uses_ai: s.swap_uses_ai,
        }
    }
}

impl From<ExerciseSettings> for ex::ExerciseSettings {
    fn from(s: ExerciseSettings) -> Self {
        Self {
            show_entry: s.show_entry,
            beginner_mode: s.beginner_mode,
            default_preset: s.default_preset.into(),
            ride_minutes: s.ride_minutes,
            cadence_tolerance_rpm: s.cadence_tolerance_rpm,
            allow_half_time: s.allow_half_time,
            allow_double_time: s.allow_double_time,
            sources: s.sources.into(),
            analysis_budget: s.analysis_budget,
            allow_explicit: s.allow_explicit,
            countdown_seconds: s.countdown_seconds,
            segment_notice_seconds: s.segment_notice_seconds,
            audible_cues: s.audible_cues,
            cue_volume: s.cue_volume,
            metronome_click: s.metronome_click,
            pulse_style: s.pulse_style.into(),
            pulse_offset_ms: s.pulse_offset_ms,
            keep_screen_on: s.keep_screen_on,
            tv_enabled: s.tv_enabled,
            tv_audio: s.tv_audio.into(),
            tv_port: s.tv_port,
            tv_window_fullscreen: s.tv_window_fullscreen,
            tv_window_display: s.tv_window_display,
            tv_lyrics: s.tv_lyrics,
            course_look: s.course_look.into(),
            course_summits: s.course_summits,
            ride_always_dark: s.ride_always_dark,
            haptic_cues: s.haptic_cues,
            health_notice_seen: s.health_notice_seen,
            tempo_nudge_pct: s.tempo_nudge_pct,
            ride_history_max: s.ride_history_max,
            taste: s.taste.into(),
            bike: s.bike.into(),
            climb_steps: s.climb_steps,
            crossfade_seconds: s.crossfade_seconds,
            course_rider: s.course_rider.into(),
            ride_big_controls: s.ride_big_controls,
            ride_swap: s.ride_swap,
            swap_uses_ai: s.swap_uses_ai,
        }
    }
}

#[uniffi::export]
pub fn exercise_default_settings() -> ExerciseSettings {
    ex::ExerciseSettings::default().into()
}

#[uniffi::export]
pub fn exercise_settings_clamped(settings: ExerciseSettings) -> ExerciseSettings {
    ex::ExerciseSettings::from(settings).clamped().into()
}

#[uniffi::export]
pub fn exercise_settings_to_json(settings: ExerciseSettings) -> String {
    serde_json::to_string(&ex::ExerciseSettings::from(settings).clamped()).unwrap_or_else(|_| "{}".into())
}

/// Lenient: anything unreadable gives the defaults, values out of range are pulled in, missing fields keep
/// their defaults.
#[uniffi::export]
pub fn exercise_settings_from_json(json: String) -> ExerciseSettings {
    serde_json::from_str::<ex::ExerciseSettings>(&json).unwrap_or_default().clamped().into()
}

// ------------------------------------------------------------------ workout

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct RideInterval {
    pub work_s: u32,
    pub rest_s: u32,
    pub rounds: u8,
    pub rest_min_rpm: u16,
    pub rest_max_rpm: u16,
}

impl From<ex::Interval> for RideInterval {
    fn from(i: ex::Interval) -> Self {
        Self {
            work_s: i.work_s,
            rest_s: i.rest_s,
            rounds: i.rounds,
            rest_min_rpm: i.rest_cadence.min_rpm,
            rest_max_rpm: i.rest_cadence.max_rpm,
        }
    }
}

impl From<RideInterval> for ex::Interval {
    fn from(i: RideInterval) -> Self {
        Self {
            work_s: i.work_s,
            rest_s: i.rest_s,
            rounds: i.rounds,
            rest_cadence: ex::RpmRange { min_rpm: i.rest_min_rpm, max_rpm: i.rest_max_rpm },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RideSegment {
    pub kind: SegmentKind,
    pub seconds: u32,
    pub min_rpm: u16,
    pub max_rpm: u16,
    /// 1..=10.
    pub effort: u8,
    pub position: RidePosition,
    pub interval: Option<RideInterval>,
    pub note: String,
    /// The climb's resistance steps (empty for any other part).
    #[uniffi(default = [])]
    pub steps: Vec<EffortStep>,
}

/// From `at_s` seconds into the segment the effort is `effort`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct EffortStep {
    pub at_s: u32,
    /// 1..=10.
    pub effort: u8,
}

impl From<ex::EffortStep> for EffortStep {
    fn from(s: ex::EffortStep) -> Self {
        Self { at_s: s.at_s, effort: s.effort.get() }
    }
}

impl From<EffortStep> for ex::EffortStep {
    fn from(s: EffortStep) -> Self {
        Self { at_s: s.at_s, effort: ex::Effort(s.effort) }
    }
}

impl From<ex::Segment> for RideSegment {
    fn from(s: ex::Segment) -> Self {
        let range = s.cadence();
        Self {
            kind: s.kind.into(),
            seconds: s.seconds,
            min_rpm: range.min_rpm,
            max_rpm: range.max_rpm,
            effort: s.effort.get(),
            position: s.position.into(),
            interval: s.interval.map(Into::into),
            note: s.note,
            steps: s.steps.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<RideSegment> for ex::Segment {
    fn from(s: RideSegment) -> Self {
        Self {
            kind: s.kind.into(),
            seconds: s.seconds,
            target: ex::Target::Cadence { min_rpm: s.min_rpm, max_rpm: s.max_rpm },
            effort: ex::Effort(s.effort),
            position: s.position.into(),
            interval: s.interval.map(Into::into),
            note: s.note,
            steps: s.steps.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum WorkoutOrigin {
    Preset { preset: PresetId },
    Ai { wish: String, provider_id: String, model: String },
    Custom,
}

impl From<ex::WorkoutOrigin> for WorkoutOrigin {
    fn from(o: ex::WorkoutOrigin) -> Self {
        match o {
            ex::WorkoutOrigin::Preset { preset } => Self::Preset { preset: preset.into() },
            ex::WorkoutOrigin::Ai { wish, provider_id, model } => Self::Ai { wish, provider_id, model },
            ex::WorkoutOrigin::Custom => Self::Custom,
        }
    }
}

impl From<WorkoutOrigin> for ex::WorkoutOrigin {
    fn from(o: WorkoutOrigin) -> Self {
        match o {
            WorkoutOrigin::Preset { preset } => Self::Preset { preset: preset.into() },
            WorkoutOrigin::Ai { wish, provider_id, model } => Self::Ai { wish, provider_id, model },
            WorkoutOrigin::Custom => Self::Custom,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RideWorkout {
    pub id: String,
    pub name: String,
    pub segments: Vec<RideSegment>,
    pub origin: WorkoutOrigin,
}

impl From<ex::Workout> for RideWorkout {
    fn from(w: ex::Workout) -> Self {
        Self {
            id: w.id,
            name: w.name,
            segments: w.segments.into_iter().map(Into::into).collect(),
            origin: w.origin.into(),
        }
    }
}

impl From<RideWorkout> for ex::Workout {
    fn from(w: RideWorkout) -> Self {
        Self {
            id: w.id,
            name: w.name,
            segments: w.segments.into_iter().map(Into::into).collect(),
            origin: w.origin.into(),
            ..ex::Workout::default()
        }
    }
}

/// The rider's taste for a ride (cleaned by the core: known decades, at most ten names each).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RideTaste {
    /// Decades by their first year (1960..=2020); empty = any era.
    pub eras: Vec<u16>,
    pub popularity: Popularity,
    /// Artists the rider likes: they lead the AI's picks.
    pub artists: Vec<String>,
    /// Artists never to play.
    pub avoid: Vec<String>,
}

impl From<ex::RideTaste> for RideTaste {
    fn from(t: ex::RideTaste) -> Self {
        Self { eras: t.eras, popularity: t.popularity.into(), artists: t.artists, avoid: t.avoid }
    }
}

impl From<RideTaste> for ex::RideTaste {
    fn from(t: RideTaste) -> Self {
        Self { eras: t.eras, popularity: t.popularity.into(), artists: t.artists, avoid: t.avoid }
    }
}

/// The decades the setup offers (the year each starts), oldest first.
#[uniffi::export]
pub fn exercise_decades() -> Vec<u16> {
    ex::DECADES.to_vec()
}

/// The taste as the core keeps it (cleaned), for the setup to show what will be used.
#[uniffi::export]
pub fn exercise_taste_clamped(taste: RideTaste) -> RideTaste {
    ex::RideTaste::from(taste).clamped().into()
}

/// Artists to offer under "artists you like": the ones the library has the most songs of in `genres`
/// (any genre when empty), at most [`ex::MAX_ARTIST_SUGGESTIONS`].
#[uniffi::export]
pub fn exercise_artist_suggestions(library: Vec<LibraryRef>, genres: Vec<String>) -> Vec<String> {
    ex::artist_suggestions(library.iter().map(|l| (l.artist.as_str(), l.genre.as_str())), &genres)
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MusicWish {
    pub genres: Vec<String>,
    /// Several genres spread over the segments by tempo.
    pub mix: bool,
    pub sentence: String,
    pub taste: RideTaste,
}

impl From<ex::MusicWish> for MusicWish {
    fn from(w: ex::MusicWish) -> Self {
        Self { genres: w.genres, mix: w.mix, sentence: w.sentence, taste: w.taste.into() }
    }
}

impl From<MusicWish> for ex::MusicWish {
    fn from(w: MusicWish) -> Self {
        Self { genres: w.genres, mix: w.mix, sentence: w.sentence, taste: w.taste.into() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RideRequest {
    /// Any length from 10 to 90, rounded to 5.
    pub minutes: u16,
    pub style: PresetId,
    pub wish: MusicWish,
    /// The rider's or the AI's own workout; `None` builds `style` at `minutes`.
    pub workout: Option<RideWorkout>,
}

impl From<ex::RideRequest> for RideRequest {
    fn from(r: ex::RideRequest) -> Self {
        Self { minutes: r.minutes, style: r.style.into(), wish: r.wish.into(), workout: r.workout.map(Into::into) }
    }
}

impl From<RideRequest> for ex::RideRequest {
    fn from(r: RideRequest) -> Self {
        Self { minutes: r.minutes, style: r.style.into(), wish: r.wish.into(), workout: r.workout.map(Into::into) }
    }
}

/// A style at a length, with the beginner caps when the settings ask for them.
#[uniffi::export]
pub fn exercise_preset(style: PresetId, minutes: u16, settings: ExerciseSettings) -> RideWorkout {
    ex::preset(style.into(), minutes, &settings.into()).into()
}

/// The workout with the beginner caps applied (the same workout when `beginner_mode` is off).
#[uniffi::export]
pub fn exercise_beginner_caps(workout: RideWorkout, settings: ExerciseSettings) -> RideWorkout {
    ex::beginner_caps(&workout.into(), &settings.into()).into()
}

/// An empty request: the settings' style and length, no wish.
#[uniffi::export]
pub fn exercise_default_request(settings: ExerciseSettings) -> RideRequest {
    let s = ex::ExerciseSettings::from(settings).clamped();
    ex::RideRequest { minutes: s.ride_minutes, style: s.default_preset, ..ex::RideRequest::default() }.into()
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PresetInfo {
    pub id: PresetId,
    /// "Easy ride", in the language asked for.
    pub name: String,
}

#[uniffi::export]
pub fn exercise_presets(language: String) -> Vec<PresetInfo> {
    let lang = Lang::from_code(&language);
    ex::PresetId::ALL.into_iter().map(|id| PresetInfo { id: id.into(), name: ex::style_name(lang, id) }).collect()
}

/// The ride minutes the picker offers: 10, 15, ... 90.
#[uniffi::export]
pub fn exercise_ride_minutes() -> Vec<u16> {
    (ex::MIN_RIDE_MINUTES..=ex::MAX_RIDE_MINUTES).step_by(usize::from(ex::RIDE_MINUTES_STEP)).collect()
}

#[uniffi::export]
pub fn exercise_health_notice(language: String) -> String {
    ex::health_notice(Lang::from_code(&language))
}

#[uniffi::export]
pub fn exercise_needs_ai_message(language: String) -> String {
    ex::needs_ai_message(Lang::from_code(&language))
}

// --------------------------------------------------------------------- plan

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct RideFit {
    pub ratio: BeatRatio,
    /// The cadence the song gives at that ratio.
    pub rpm: f32,
    /// How far outside the target range (0 inside).
    pub error_rpm: f32,
}

impl From<ex::Fit> for RideFit {
    fn from(f: ex::Fit) -> Self {
        Self { ratio: f.ratio.into(), rpm: f.rpm, error_rpm: f.error_rpm }
    }
}

impl From<RideFit> for ex::Fit {
    fn from(f: RideFit) -> Self {
        Self { ratio: f.ratio.into(), rpm: f.rpm, error_rpm: f.error_rpm }
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PlannedSong {
    pub segment_index: u32,
    pub entry: PlaylistEntry,
    pub bpm: Option<u16>,
    pub source: Option<BpmSource>,
    pub fit: Option<RideFit>,
    pub grid: Option<DjGrid>,
    pub pulse: PulseSource,
    /// The song is cut at this position (ms): go to the next song then (see `RideView::skip_to_next`).
    #[uniffi(default = None)]
    pub play_ms: Option<u64>,
    /// The song's position 0 is this instant of its segment (ms): a song swapped in mid-part starts from its
    /// beginning while the part's clock goes on. 0 for a song of a plan as built.
    #[uniffi(default = 0)]
    pub at_segment_ms: u64,
}

impl From<ex::PlannedSong> for PlannedSong {
    fn from(s: ex::PlannedSong) -> Self {
        Self {
            segment_index: count(s.segment_index),
            entry: s.entry.into(),
            bpm: s.bpm,
            source: s.source.map(Into::into),
            fit: s.fit.map(Into::into),
            grid: s.grid.map(Into::into),
            pulse: s.pulse.into(),
            play_ms: s.play_ms,
            at_segment_ms: s.at_segment_ms,
        }
    }
}

impl From<PlannedSong> for ex::PlannedSong {
    fn from(s: PlannedSong) -> Self {
        Self {
            segment_index: s.segment_index as usize,
            entry: s.entry.into(),
            bpm: s.bpm,
            source: s.source.map(Into::into),
            fit: s.fit.map(Into::into),
            grid: s.grid.and_then(|g| Grid::new(g.first_beat_ms, g.beat_ms)),
            pulse: s.pulse.into(),
            play_ms: s.play_ms,
            at_segment_ms: s.at_segment_ms,
        }
    }
}

/// A point of the course profile: seconds into the ride and elevation 0..=1.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct ProfilePoint {
    pub t_s: u32,
    pub elevation: f32,
}

impl From<ex::ProfilePoint> for ProfilePoint {
    fn from(p: ex::ProfilePoint) -> Self {
        Self { t_s: p.t_s, elevation: p.elevation }
    }
}

impl From<ProfilePoint> for ex::ProfilePoint {
    fn from(p: ProfilePoint) -> Self {
        Self { t_s: p.t_s, elevation: p.elevation }
    }
}

/// The ride: segments snapped to the songs, one song per segment, in play order.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RidePlan {
    pub workout: RideWorkout,
    pub songs: Vec<PlannedSong>,
    /// Why the ride is not exactly what was asked, in the build language.
    pub notes: Vec<String>,
    /// The AI was asked and failed (why): the apps show it loudly. None when it answered or was not asked.
    pub ai_problem: Option<String>,
    pub countdown_seconds: u8,
    pub notice_seconds: u8,
    pub wish: MusicWish,
    pub profile: Vec<ProfilePoint>,
}

impl From<ex::RidePlan> for RidePlan {
    fn from(p: ex::RidePlan) -> Self {
        Self {
            workout: p.workout.into(),
            songs: p.songs.into_iter().map(Into::into).collect(),
            notes: p.notes,
            ai_problem: p.ai_problem,
            countdown_seconds: p.cues.countdown_seconds,
            notice_seconds: p.cues.notice_seconds,
            wish: p.wish.into(),
            profile: p.profile.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<RidePlan> for ex::RidePlan {
    fn from(p: RidePlan) -> Self {
        let workout = ex::Workout::from(p.workout);
        // the workout of a plan keeps its id and origin; the profile is rebuilt when the caller sent none
        let profile =
            if p.profile.is_empty() { ex::profile(&workout) } else { p.profile.into_iter().map(Into::into).collect() };
        Self {
            version: 1,
            workout,
            songs: p.songs.into_iter().map(Into::into).collect(),
            notes: p.notes,
            ai_problem: p.ai_problem,
            cues: ex::CueSettings { countdown_seconds: p.countdown_seconds, notice_seconds: p.notice_seconds },
            wish: p.wish.into(),
            profile,
        }
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RideBuilt {
    pub plan: RidePlan,
    /// What the player plays, in the plan's order. Not saved: `home_playlist_save` on start.
    pub playlist: HomePlaylist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum RideProgress {
    Searching { with_ai: bool },
    Resolving { done: u32, total: u32 },
    Analyzing { done: u32, total: u32 },
    Matching,
}

impl From<ex::RideProgress> for RideProgress {
    fn from(p: ex::RideProgress) -> Self {
        match p {
            ex::RideProgress::Searching { with_ai } => Self::Searching { with_ai },
            ex::RideProgress::Resolving { done, total } => Self::Resolving { done: count(done), total: count(total) },
            ex::RideProgress::Analyzing { done, total } => Self::Analyzing { done: count(done), total: count(total) },
            ex::RideProgress::Matching => Self::Matching,
        }
    }
}

/// Told as the build advances (called from a background thread).
#[uniffi::export(callback_interface)]
pub trait RideListener: Send + Sync {
    fn on_progress(&self, progress: RideProgress);
}

// ----------------------------------------------------------------- the view

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct RideIntervalView {
    pub phase: IntervalPhase,
    /// 1-based.
    pub round: u8,
    pub rounds: u8,
    pub phase_remaining_s: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct RideNext {
    pub kind: SegmentKind,
    pub position: RidePosition,
    pub target_rpm: u16,
    /// The target in the bike's unit.
    pub shown: ShownTarget,
    /// The knob position of the next part's starting effort; `None` when the bike's levels are unknown.
    pub level: Option<u8>,
    pub seconds: u32,
}

/// Where the rider is inside a climb's resistance steps: "level 2 of 3".
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct StepView {
    /// 1-based; the climb's own start is 1, so after the first step it is 2 ("Resistance +1" chip).
    pub index: u8,
    pub count: u8,
    /// The effort at this level.
    pub effort: u8,
    /// Seconds to the next step, `None` on the last level.
    pub next_in_s: Option<u32>,
}

/// A resistance step on the course mountain: `t_s` seconds into the ride; the renderer reads the ridge's
/// elevation there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct HotPoint {
    pub t_s: u32,
    /// The effort from here on.
    pub effort: u8,
    /// 1-based level of the climb that starts here (the first hot point is level 2).
    pub level: u8,
    /// Levels of the climb.
    pub count: u8,
}

impl From<ex::HotPoint> for HotPoint {
    fn from(p: ex::HotPoint) -> Self {
        Self { t_s: p.t_s, effort: p.effort, level: p.level, count: p.count }
    }
}

impl From<HotPoint> for ex::HotPoint {
    fn from(p: HotPoint) -> Self {
        Self { t_s: p.t_s, effort: p.effort, level: p.level, count: p.count }
    }
}

/// Every resistance step of a workout, in ride order (the markers on the mountain).
#[uniffi::export]
pub fn exercise_hot_points(workout: RideWorkout) -> Vec<HotPoint> {
    ex::hot_points(&ex::Workout::from(workout)).into_iter().map(Into::into).collect()
}

/// The beat the rider pedals to: stroke `n` is at `first_beat_ms + n * stroke_ms` of the song.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct RidePulse {
    pub source: PulseSource,
    pub first_beat_ms: f64,
    pub beat_ms: f64,
    pub stroke_ms: f64,
    pub ratio: BeatRatio,
    pub song_position_ms: u64,
}

/// A sound to play `in_ms` from the view's moment (negative: just passed). `id` is stable across views.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct RideCue {
    pub id: u32,
    pub kind: CueKind,
    pub in_ms: i64,
}

/// What the ride screen shows right now.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RideView {
    pub at_ms: u64,
    pub finished: bool,
    pub segment_index: u32,
    pub segment_count: u32,
    pub kind: SegmentKind,
    pub position: RidePosition,
    /// The effort now (a climb's steps raise it).
    pub effort: u8,
    /// The big number, in RPM (`shown.rpm`).
    pub target_rpm: u16,
    pub min_rpm: u16,
    pub max_rpm: u16,
    /// The target in the bike's unit (km/h once calibrated): what the ring shows.
    pub shown: ShownTarget,
    /// The bike's knob position for `effort`; `None` when its levels are unknown.
    pub level: Option<u8>,
    /// How many positions the knob has (0 when `level` is `None`), for "6/8".
    pub level_count: u8,
    /// The resistance step the rider is on; `None` for a part without steps.
    pub step: Option<StepView>,
    pub segment_seconds: u32,
    pub segment_start_s: u32,
    pub remaining_s: u32,
    pub segment_progress: f32,
    pub elapsed_s: u32,
    pub total_s: u32,
    pub progress: f32,
    /// Where the rider is on the course profile.
    pub marker: ProfilePoint,
    pub interval: Option<RideIntervalView>,
    /// The change is near: show "next".
    pub notice: bool,
    pub next: Option<RideNext>,
    pub pulse: RidePulse,
    pub cues: Vec<RideCue>,
    /// The song reached its cut: skip to the next one now (the ride's segment ended before the song did).
    pub skip_to_next: bool,
}

impl From<ex::RideView> for RideView {
    fn from(v: ex::RideView) -> Self {
        Self {
            at_ms: v.at_ms,
            finished: v.finished,
            segment_index: v.segment_index,
            segment_count: v.segment_count,
            kind: v.kind.into(),
            position: v.position.into(),
            effort: v.effort,
            target_rpm: v.target_rpm,
            min_rpm: v.range.min_rpm,
            max_rpm: v.range.max_rpm,
            shown: v.shown.into(),
            level: v.level,
            level_count: v.level_count,
            step: v.step.map(|s| StepView { index: s.index, count: s.count, effort: s.effort, next_in_s: s.next_in_s }),
            segment_seconds: v.segment_seconds,
            segment_start_s: v.segment_start_s,
            remaining_s: v.remaining_s,
            segment_progress: v.segment_progress,
            elapsed_s: v.elapsed_s,
            total_s: v.total_s,
            progress: v.progress,
            marker: v.marker.into(),
            interval: v.interval.map(|i| RideIntervalView {
                phase: i.phase.into(),
                round: i.round,
                rounds: i.rounds,
                phase_remaining_s: i.phase_remaining_s,
            }),
            notice: v.notice,
            next: v.next.map(|n| RideNext {
                kind: n.kind.into(),
                position: n.position.into(),
                target_rpm: n.target_rpm,
                shown: n.shown.into(),
                level: n.level,
                seconds: n.seconds,
            }),
            pulse: RidePulse {
                source: v.pulse.source.into(),
                first_beat_ms: v.pulse.first_beat_ms,
                beat_ms: v.pulse.beat_ms,
                stroke_ms: v.pulse.stroke_ms,
                ratio: v.pulse.ratio.into(),
                song_position_ms: v.pulse.song_position_ms,
            },
            cues: v.cues.into_iter().map(|c| RideCue { id: c.id, kind: c.kind.into(), in_ms: c.in_ms }).collect(),
            skip_to_next: v.skip_to_next,
        }
    }
}

/// The ride's plan kept in Rust: the 10 Hz view then converts nothing but the view itself.
#[derive(uniffi::Object)]
pub struct ExerciseRide {
    plan: Arc<ex::RidePlan>,
    /// The rider's bike: targets in its console's unit, efforts as knob positions.
    bike: ex::BikeProfile,
}

#[uniffi::export]
impl ExerciseRide {
    /// `bike` `None` = no bike set up (targets in RPM, no levels). A swap or a changed bike: make a new one.
    #[uniffi::constructor(default(bike = None))]
    pub fn new(plan: RidePlan, bike: Option<BikeProfile>) -> Arc<Self> {
        Arc::new(Self { plan: Arc::new(plan.into()), bike: bike.map(Into::into).unwrap_or_default() })
    }

    /// What the screen shows at `queue_index` (the playing song of the ride's queue) and `position_ms`
    /// (its position). Pure: a paused player gives the same view.
    pub fn view(&self, queue_index: u32, position_ms: u64, now_ms: u64) -> RideView {
        ex::RideClock::view_on(&self.plan, &self.bike, queue_index as usize, position_ms, now_ms).into()
    }

    /// The parts of the ride for [`TvSession::parts`] (the mountain's colours).
    pub fn tv_parts(&self) -> Vec<TvPart> {
        ex::TvPart::of(&self.plan.workout).into_iter().map(Into::into).collect()
    }

    /// The resistance steps of the ride, for the mountain's markers.
    pub fn hot_points(&self) -> Vec<HotPoint> {
        ex::hot_points(&self.plan.workout).into_iter().map(Into::into).collect()
    }

    /// The cues of the view still to come, as `(at_ms, kind)` on the view's clock.
    pub fn schedule(&self, queue_index: u32, position_ms: u64, now_ms: u64) -> Vec<CueAt> {
        let view = ex::RideClock::view_on(&self.plan, &self.bike, queue_index as usize, position_ms, now_ms);
        ex::schedule(&view).into_iter().map(|(at_ms, kind)| CueAt { at_ms, kind: kind.into() }).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct CueAt {
    pub at_ms: u64,
    pub kind: CueKind,
}

/// [`ExerciseRide::view`] without keeping the plan (converts it every call: prefer the object).
#[uniffi::export(default(bike = None))]
pub fn exercise_ride_view(
    plan: RidePlan,
    queue_index: u32,
    position_ms: u64,
    now_ms: u64,
    bike: Option<BikeProfile>,
) -> RideView {
    let bike = bike.map(Into::into).unwrap_or_default();
    ex::RideClock::view_on(&ex::RidePlan::from(plan), &bike, queue_index as usize, position_ms, now_ms).into()
}

/// A cue as a mono 16-bit WAV file's bytes (the app writes it once and plays it with `SoundPool`).
#[uniffi::export]
pub fn exercise_cue_wav(kind: CueKind, sample_rate: u32) -> Vec<u8> {
    ex::cues::wav(kind.into(), sample_rate)
}

// -------------------------------------------------------------------- build

/// Reads the tempo of a local song with the DJ's analysis (and its cache); Tidal-only songs are not decoded.
struct FileAnalyzer {
    dir: PathBuf,
    memory_mb: u32,
}

impl ex::Analyzer for FileAnalyzer {
    fn can_read(&self, entry: &pl::PlaylistEntry) -> bool {
        entry.local_path.is_some()
    }

    fn analyze<'a>(
        &'a self,
        entry: &'a pl::PlaylistEntry,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<CoreAnalysis>> + Send + 'a>> {
        Box::pin(async move {
            let path = entry.local_path.clone()?;
            let (dir, memory_mb) = (self.dir.clone(), self.memory_mb);
            tokio::task::spawn_blocking(move || crate::dj::analyze_cached(Path::new(&path), dir, memory_mb).ok())
                .await
                .ok()
                .flatten()
        })
    }
}

/// What a build or a swap works with, owned for the length of the call (the core borrows it).
struct RideEnv {
    api: Option<homefi_core::api::TidalApi>,
    ai: Option<homefi_core::ai::AiConfig>,
    index: homefi_core::playlist::LibraryIndex,
    settings: ex::ExerciseSettings,
    analyzer: FileAnalyzer,
}

impl RideEnv {
    async fn new(
        engine: &HomefiEngine,
        settings: ExerciseSettings,
        ai: Option<AiConfig>,
        api_key: Option<String>,
        library: Vec<LibraryRef>,
    ) -> Self {
        // signed out is not an error: only the library and the AI are used
        let _ = engine.session.refresh(600, false).await;
        Self {
            api: engine.session.api().ok(),
            ai: ai.map(|c| core_config(c, api_key)),
            index: library_index(library).await,
            settings: ex::ExerciseSettings::from(settings),
            analyzer: FileAnalyzer {
                dir: ANALYSIS_DIR.iter().fold(engine.session.app_dir().to_path_buf(), |p, part| p.join(part)),
                memory_mb: homefi_core::dj::settings::ANDROID_MEMORY_MB,
            },
        }
    }

    fn ctx(&self, language: &str) -> ex::BuildContext<'_> {
        ex::BuildContext {
            settings: &self.settings,
            ai: self.ai.as_ref(),
            api: self.api.as_ref(),
            library: &self.index,
            analyzer: Some(&self.analyzer),
            lang: Lang::from_code(language),
        }
    }
}

#[uniffi::export]
impl HomefiEngine {
    /// Builds the ride (not saved: the app saves `playlist` when the ride starts). The AI's key travels apart
    /// and is never stored. Cancelling the coroutine stops the work between songs.
    /// Every build searches everything; a build of the same ride within half an hour tells the AI what it
    /// proposed before (so it picks others) and puts the songs already heard last.
    #[allow(clippy::too_many_arguments)]
    pub async fn exercise_build(
        self: Arc<Self>,
        request: RideRequest,
        settings: ExerciseSettings,
        ai: Option<AiConfig>,
        api_key: Option<String>,
        language: String,
        library: Vec<LibraryRef>,
        listener: Box<dyn RideListener>,
    ) -> Result<RideBuilt> {
        crate::runtime::run(async move {
            let env = RideEnv::new(&self, settings, ai, api_key, library).await;
            // what earlier builds of this ride proposed and played, for "new songs"
            let memory = self.ride_memory();
            let (built, memory) = ex::build_with_memory(
                &request.into(),
                &env.ctx(&language),
                &|p| listener.on_progress(p.into()),
                memory,
            )
            .await?;
            self.remember_ride(memory);
            Ok(RideBuilt { plan: built.plan.into(), playlist: built.playlist.into() })
        })
        .await
    }

    /// Another song for the part `segment_index` (the song's index in `built.plan.songs`, the same as in the
    /// playlist) of a built ride; `request` is the one it was built from, so the memory of that build (the
    /// songs it found, so there is no search) is used. `at_segment_ms` is how far into the part the rider is
    /// (0 in the plan preview): the new song then starts from its beginning (`PlannedSong::at_segment_ms`) and
    /// must cover what is left of the part. The song sent away never comes back to this ride. Returns the
    /// whole ride with the swapped song, its playlist and its profile: the app saves the playlist and replaces
    /// the queue's item. `settings.ride_swap` off is an error; `settings.swap_uses_ai` off keeps the AI out
    /// of it (the library and Tidal are searched when nothing already found fits).
    #[allow(clippy::too_many_arguments)]
    pub async fn exercise_swap(
        self: Arc<Self>,
        request: RideRequest,
        built: RideBuilt,
        segment_index: u32,
        at_segment_ms: u64,
        settings: ExerciseSettings,
        ai: Option<AiConfig>,
        api_key: Option<String>,
        language: String,
        library: Vec<LibraryRef>,
        listener: Box<dyn RideListener>,
    ) -> Result<RideBuilt> {
        crate::runtime::run(async move {
            let env = RideEnv::new(&self, settings, ai, api_key, library).await;
            let built = ex::RideBuilt { plan: built.plan.into(), playlist: built.playlist.into() };
            let req = ex::SwapRequest { built: &built, segment_index: segment_index as usize, at_segment_ms };
            let (swapped, memory) = ex::swap_song(
                &req,
                &request.into(),
                &env.ctx(&language),
                &|p| listener.on_progress(p.into()),
                self.ride_memory(),
            )
            .await?;
            self.remember_ride(memory);
            Ok(RideBuilt { plan: swapped.built.plan.into(), playlist: swapped.built.playlist.into() })
        })
        .await
    }
}

// --------------------------------------------------------------------- chat

/// What the AI says in the chat, and the request as it now understands it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ExerciseTurn {
    pub text: String,
    pub quick_replies: Vec<String>,
    pub request: RideRequest,
    /// The AI's short name for the ride (empty until it sets one).
    pub title: String,
}

impl From<ex::ExerciseTurn> for ExerciseTurn {
    fn from(t: ex::ExerciseTurn) -> Self {
        Self { text: t.text, quick_replies: t.quick_replies, request: t.request.into(), title: t.title }
    }
}

/// The optional chat before a build. Turns never overlap (one tokio mutex).
#[derive(uniffi::Object)]
pub struct ExerciseSession {
    inner: Arc<tokio::sync::Mutex<ex::ExerciseSession>>,
}

#[uniffi::export]
impl ExerciseSession {
    #[uniffi::constructor]
    pub fn new(config: AiConfig, api_key: Option<String>, request: RideRequest, language: String) -> Arc<Self> {
        let session =
            ex::ExerciseSession::new(core_config(config, api_key), request.into(), Lang::from_code(&language));
        Arc::new(Self { inner: Arc::new(tokio::sync::Mutex::new(session)) })
    }

    /// HomeFi's fixed opener and example answers: no network.
    pub async fn start(&self) -> ExerciseTurn {
        self.inner.lock().await.start().into()
    }

    pub async fn reply(&self, text: String) -> Result<ExerciseTurn> {
        let inner = self.inner.clone();
        crate::runtime::run(async move { Ok(inner.lock().await.reply(&text).await?.into()) }).await
    }
}

// ----------------------------------------------------------------------- TV

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RideListen {
    /// The ride hub's port and its one-ride join code (a listen-only guest).
    pub port: u16,
    pub code: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TvText {
    pub segment: String,
    pub position: String,
    /// "Hard 7", or "Hard 7 · level 6/8" when the bike's levels are known.
    pub effort: String,
    pub next: String,
    /// The unit the target is shown in: "RPM", "km/h" or "mph".
    #[uniffi(default = "")]
    pub unit: String,
    /// "Resistance +1 · 2 of 3" once a resistance step has started, else empty.
    #[uniffi(default = "")]
    pub step: String,
}

/// A part of the ride under the TV's mountain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct TvPart {
    pub kind: SegmentKind,
    pub start_s: u32,
    pub seconds: u32,
}

impl From<ex::TvPart> for TvPart {
    fn from(p: ex::TvPart) -> Self {
        Self { kind: p.kind.into(), start_s: p.start_s, seconds: p.seconds }
    }
}

/// One line of the song on the TV.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TvLyricLine {
    pub t_ms: u64,
    pub text: String,
}

/// The lyrics the TV carries for the song that plays (capped by the core).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TvLyrics {
    pub song_key: String,
    pub synced: bool,
    pub offset_ms: i32,
    pub duration_ms: u64,
    pub lines: Vec<TvLyricLine>,
}

impl From<ex::tv::TvLyrics> for TvLyrics {
    fn from(l: ex::tv::TvLyrics) -> Self {
        Self {
            song_key: l.song_key,
            synced: l.synced,
            offset_ms: l.offset_ms,
            duration_ms: l.duration_ms,
            lines: l.lines.into_iter().map(|l| TvLyricLine { t_ms: l.t_ms, text: l.text }).collect(),
        }
    }
}

/// The JSON of `/ride.json`, parsed: what an Android TV client draws.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RideTvView {
    pub v: u8,
    pub server_now_ms: u64,
    /// When `ride.pulse.song_position_ms` was read, on the server's clock.
    pub sampled_ms: u64,
    pub playing: bool,
    pub title: String,
    pub audio: TvAudio,
    pub listen: Option<RideListen>,
    pub profile: Vec<ProfilePoint>,
    pub parts: Vec<TvPart>,
    pub ride: Option<RideView>,
    pub text: TvText,
    pub pulse_style: PulseStyle,
    /// How the TV draws the course mountain.
    pub course_look: CourseLook,
    pub course_summits: bool,
    /// The resistance steps of the climbs, marked on the mountain.
    pub hot_points: Vec<HotPoint>,
    /// How the TV draws the rider on the mountain.
    pub course_rider: CourseRider,
    /// A ride runs and its music is paused.
    pub paused: bool,
    /// The 3-2-1 digit showing when the position was sampled (0 = none).
    pub countdown_s: u8,
    /// The lyrics of the song that plays, when the server has them.
    pub lyrics: Option<TvLyrics>,
}

impl From<ex::RideTvView> for RideTvView {
    fn from(v: ex::RideTvView) -> Self {
        Self {
            v: v.v,
            server_now_ms: v.server_now_ms,
            sampled_ms: v.sampled_ms,
            playing: v.playing,
            title: v.title,
            audio: v.audio.into(),
            listen: v.listen.map(|l| RideListen { port: l.port, code: l.code, name: l.name }),
            profile: v.profile.into_iter().map(Into::into).collect(),
            parts: v.parts.into_iter().map(Into::into).collect(),
            ride: v.ride.map(Into::into),
            text: TvText {
                segment: v.text.segment,
                position: v.text.position,
                effort: v.text.effort,
                next: v.text.next,
                unit: v.text.unit,
                step: v.text.step,
            },
            pulse_style: v.pulse_style.into(),
            course_look: v.course_look.into(),
            course_summits: v.course_summits,
            hot_points: v.hot_points.into_iter().map(Into::into).collect(),
            course_rider: v.course_rider.into(),
            paused: v.paused,
            countdown_s: v.countdown_s,
            lyrics: v.lyrics.map(Into::into),
        }
    }
}

/// A ride TV found on the network.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RideTvEndpoint {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub version: u8,
    /// `http://host:port/ride.json`
    pub ride_url: String,
}

/// Looks for ride TVs on the network for `timeout_ms` (mDNS `_homefi-ride._tcp`; the app must hold a
/// multicast lock).
#[uniffi::export]
pub async fn exercise_tv_discover(timeout_ms: u32) -> Vec<RideTvEndpoint> {
    crate::runtime::run(async move {
        ex::discover(Duration::from_millis(u64::from(timeout_ms)))
            .await
            .into_iter()
            .map(|e| {
                let ride_url = e.ride_url();
                RideTvEndpoint { name: e.name, host: e.host, port: e.port, version: e.version, ride_url }
            })
            .collect()
    })
    .await
}

/// Reads a `/ride.json` answer. A newer protocol than this app knows is an error.
#[uniffi::export]
pub fn exercise_tv_parse(json: String) -> Result<RideTvView> {
    Ok(ex::RideTvView::from_json(&json)?.into())
}

/// The protocol version this build reads and writes.
#[uniffi::export]
pub fn exercise_tv_protocol_version() -> u8 {
    ex::TV_PROTOCOL_VERSION
}

/// A fresh one-ride join code for the listen-only hub of audio `Tv`.
#[uniffi::export]
pub fn exercise_ride_token() -> String {
    ex::tv::ride_token()
}

/// The ride hub's rules: guests only listen (no adding, skipping, pausing or editing). Pass it, and
/// [`exercise_ride_token`] as the code, with `stream_audio` on, to `start_hub`.
#[uniffi::export]
pub fn exercise_listen_permissions() -> SharePermissions {
    ex::tv::listen_permissions().into()
}

/// What stays the same during a ride, for the TV.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct TvSession {
    pub title: String,
    pub profile: Vec<ProfilePoint>,
    /// `ExerciseRide::tv_parts`.
    pub parts: Vec<TvPart>,
    pub audio: TvAudio,
    /// The ride hub, when the audio goes to the TV; `None` otherwise.
    pub listen: Option<RideListen>,
    /// How the TV draws the beat.
    pub pulse_style: PulseStyle,
    /// How the TV draws the course mountain (`None` = ribbon; uniffi cannot default an enum).
    #[uniffi(default = None)]
    pub course_look: Option<CourseLook>,
    /// The mountain labels its summits with their time.
    #[uniffi(default = false)]
    pub course_summits: bool,
    /// `ExerciseRide::hot_points`: the resistance steps marked on the TV's mountain.
    #[uniffi(default = [])]
    pub hot_points: Vec<HotPoint>,
    /// How the TV draws the rider (`None` = the bike, the default).
    #[uniffi(default = None)]
    pub course_rider: Option<CourseRider>,
}

/// The ride TV server: the page, `/ride.json` and the mDNS announcement. One per app.
#[derive(uniffi::Object)]
pub struct ExerciseTv {
    server: Mutex<Option<ex::TvServer>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[uniffi::export]
impl ExerciseTv {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self { server: Mutex::new(None) })
    }

    /// Starts serving on `port` (the next one if it is in use) and returns the URL a TV browser opens.
    /// `announce` also announces `_homefi-ride._tcp` for Android TV clients.
    pub async fn start(self: Arc<Self>, port: u16, name: String, announce: bool, language: String) -> Result<String> {
        crate::runtime::run(async move {
            self.clone().stop().await;
            let config = ex::tv::TvConfig { port, name, lang: Lang::from_code(&language), announce };
            let server = ex::TvServer::start(config).await.map_err(HomefiError::from)?;
            let url = server.url().unwrap_or_else(|| format!("http://127.0.0.1:{}/", server.port()));
            *lock(&self.server) = Some(server);
            Ok(url)
        })
        .await
    }

    /// A ride starts: its title, course profile and how the audio goes.
    pub fn begin(&self, session: TvSession) {
        if let Some(server) = lock(&self.server).as_ref() {
            server.begin(ex::tv::TvSession {
                title: session.title,
                profile: session.profile.into_iter().map(Into::into).collect(),
                parts: session
                    .parts
                    .into_iter()
                    .map(|p| ex::TvPart { kind: p.kind.into(), start_s: p.start_s, seconds: p.seconds })
                    .collect(),
                audio: session.audio.into(),
                listen: session.listen.map(|l| ex::tv::RideListen { port: l.port, code: l.code, name: l.name }),
                pulse_style: session.pulse_style.into(),
                course_look: session.course_look.map(Into::into).unwrap_or_default(),
                course_summits: session.course_summits,
                hot_points: session.hot_points.into_iter().map(Into::into).collect(),
                course_rider: session.course_rider.map(Into::into).unwrap_or_default(),
            });
        }
    }

    /// Publishes the ride's state as the phone shows it (the same clock), with `playing` false when paused.
    pub fn publish(&self, ride: Arc<ExerciseRide>, queue_index: u32, position_ms: u64, now_ms: u64, playing: bool) {
        if let Some(server) = lock(&self.server).as_ref() {
            server.publish(
                ex::RideClock::view_on(&ride.plan, &ride.bike, queue_index as usize, position_ms, now_ms),
                playing,
            );
        }
    }

    /// The lyrics of the song that plays now, for the TV page to follow along (`offset_ms` > 0 shows them later;
    /// `duration_ms` is the song's length, 0 = unknown). `None`, or lyrics without text, clears them: call it
    /// whenever the ride's song changes (then again with the lyrics once they are loaded) and when the
    /// `tv_lyrics` setting is off. The core caps their size.
    pub fn set_lyrics(&self, song_key: String, lyrics: Option<crate::SongLyrics>, offset_ms: i32, duration_ms: u64) {
        if let Some(server) = lock(&self.server).as_ref() {
            let tv = lyrics.and_then(|l| {
                let core = homefi_core::lyrics::Lyrics {
                    lines: l
                        .lines
                        .into_iter()
                        .map(|l| homefi_core::lyrics::LyricLine { time_ms: l.time_ms, text: l.text })
                        .collect(),
                    synced: l.synced,
                    provider: l.provider,
                };
                ex::tv::TvLyrics::new(&song_key, &core, offset_ms, duration_ms)
            });
            server.set_lyrics(tv);
        }
    }

    /// The ride is over: the page goes back to waiting.
    pub fn clear(&self) {
        if let Some(server) = lock(&self.server).as_ref() {
            server.clear();
        }
    }

    pub fn is_running(&self) -> bool {
        lock(&self.server).is_some()
    }

    pub fn port(&self) -> Option<u16> {
        lock(&self.server).as_ref().map(ex::TvServer::port)
    }

    pub async fn stop(self: Arc<Self>) {
        let server = lock(&self.server).take();
        if let Some(server) = server {
            crate::runtime::run(server.stop()).await;
        }
    }
}

// ------------------------------------------------------------------- saved rides

/// A saved ride as a list shows it (with the profile for a mini mountain).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct SavedRideSummary {
    pub id: String,
    pub name: String,
    /// Whole minutes of the ride.
    pub minutes: u32,
    pub style: PresetId,
    pub genres: Vec<String>,
    pub songs: u32,
    pub created_ms: u64,
    pub last_ridden_ms: Option<u64>,
    pub times_ridden: u32,
    pub favorite: bool,
    /// Saved by itself after a build (it may be pruned); `false` once kept.
    pub auto: bool,
    pub profile: Vec<ProfilePoint>,
}

impl From<&ex::SavedRide> for SavedRideSummary {
    fn from(r: &ex::SavedRide) -> Self {
        let s = r.summary();
        Self {
            id: s.id,
            name: s.name,
            minutes: s.minutes,
            style: s.style.into(),
            genres: s.genres,
            songs: count(s.songs),
            created_ms: s.created_ms,
            last_ridden_ms: s.last_ridden_ms,
            times_ridden: s.times_ridden,
            favorite: s.favorite,
            auto: s.auto,
            profile: s.profile.into_iter().map(Into::into).collect(),
        }
    }
}

/// A saved ride with everything needed to ride it again (no build, no AI).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct SavedRide {
    pub summary: SavedRideSummary,
    pub request: RideRequest,
    pub plan: RidePlan,
    /// The playlist the player played for it, when still known.
    pub playlist_id: Option<String>,
}

impl From<ex::SavedRide> for SavedRide {
    fn from(r: ex::SavedRide) -> Self {
        Self { summary: (&r).into(), request: r.request.into(), plan: r.plan.into(), playlist_id: r.playlist_id }
    }
}

/// The saved rides in `<app dir>/exercise/rides`. Every successful build is saved with
/// `auto_save`; riding one again loads its plan and starts it.
#[derive(uniffi::Object)]
pub struct ExerciseRides {
    store: ex::RideStore,
}

#[uniffi::export]
impl ExerciseRides {
    #[uniffi::constructor]
    pub fn new(app_dir: String) -> Arc<Self> {
        Arc::new(Self { store: ex::RideStore::in_app_dir(Path::new(&app_dir)) })
    }

    /// Favorites first, then the most recent.
    pub fn list(&self) -> Vec<SavedRideSummary> {
        self.store.list().iter().map(Into::into).collect()
    }

    pub fn get(&self, id: String) -> Result<SavedRide> {
        Ok(self.store.get(&id)?.into())
    }

    /// Saves a finished build automatically (named "30 min · Classic · Rock + Metal" in `language`)
    /// and prunes the history to `history_max` (`ExerciseSettings.ride_history_max`).
    pub fn auto_save(
        &self,
        request: RideRequest,
        plan: RidePlan,
        playlist_id: Option<String>,
        language: String,
        history_max: u16,
    ) -> Result<SavedRideSummary> {
        let (request, plan) = (ex::RideRequest::from(request), ex::RidePlan::from(plan));
        let saved = self.store.auto_save(&request, &plan, playlist_id, Lang::from_code(&language), history_max)?;
        Ok((&saved).into())
    }

    /// The rider keeps it (pruning leaves it), optionally naming it.
    pub fn keep(&self, id: String, name: Option<String>) -> Result<SavedRideSummary> {
        Ok((&self.store.keep(&id, name.as_deref())?).into())
    }

    pub fn rename(&self, id: String, name: String) -> Result<SavedRideSummary> {
        Ok((&self.store.rename(&id, &name)?).into())
    }

    pub fn set_favorite(&self, id: String, favorite: bool) -> Result<SavedRideSummary> {
        Ok((&self.store.set_favorite(&id, favorite)?).into())
    }

    /// A song of the saved ride was swapped: the ride keeps the new plan (and its playlist, when given).
    pub fn replace_plan(&self, id: String, plan: RidePlan, playlist_id: Option<String>) -> Result<SavedRideSummary> {
        Ok((&self.store.replace_plan(&id, &ex::RidePlan::from(plan), playlist_id)?).into())
    }

    /// Counts a ride started from this saved ride.
    pub fn mark_ridden(&self, id: String) -> Result<SavedRideSummary> {
        Ok((&self.store.mark_ridden(&id)?).into())
    }

    /// `false` when there was no such ride.
    pub fn delete(&self, id: String) -> Result<bool> {
        Ok(self.store.delete(&id)?)
    }

    /// Removes automatic rides beyond `history_max`; the number removed.
    pub fn prune(&self, history_max: u16) -> Result<u32> {
        Ok(count(self.store.prune(history_max)?.len()))
    }

    /// The `.homefiride` text (local paths cut to file names, no ids or counters).
    pub fn export(&self, id: String) -> Result<String> {
        Ok(ex::export_ride(&self.store.get(&id)?)?)
    }

    /// Saves a ride from `.homefiride` text as a new kept ride. Local songs are matched again in
    /// `library`; the others stay as Tidal songs. Wrong, damaged or newer files are refused with a
    /// message for the screen.
    pub async fn import(&self, json: String, library: Vec<LibraryRef>) -> Result<SavedRideSummary> {
        let index = library_index(library).await;
        let ride = ex::import_ride(&json, Some(&index))?;
        self.store.save(&ride)?;
        Ok((&ride).into())
    }
}

/// File extension of an exported ride (no dot).
#[uniffi::export]
pub fn exercise_ride_extension() -> String {
    homefi_core::export::RIDE_EXTENSION.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_rides_round_trip_through_the_ffi_store() {
        let tmp = tempfile::tempdir().unwrap();
        let rides = ExerciseRides::new(tmp.path().to_string_lossy().into_owned());
        let request: RideRequest = ex::RideRequest::default().into();
        let workout = ex::preset(ex::PresetId::Classic, 30, &ex::ExerciseSettings::default());
        let plan = ex::RidePlan {
            songs: vec![ex::PlannedSong {
                entry: pl::PlaylistEntry { title: "A".into(), tidal_id: Some("1".into()), ..Default::default() },
                ..Default::default()
            }],
            profile: ex::profile(&workout),
            workout,
            ..Default::default()
        };
        let saved = rides.auto_save(request, plan.into(), None, "en".into(), 30).unwrap();
        assert!(saved.auto && saved.minutes == 30);
        assert_eq!(rides.list().len(), 1);
        assert!(!rides.keep(saved.id.clone(), Some("Mine".into())).unwrap().auto);
        assert!(rides.set_favorite(saved.id.clone(), true).unwrap().favorite);
        assert_eq!(rides.mark_ridden(saved.id.clone()).unwrap().times_ridden, 1);
        assert_eq!(rides.get(saved.id.clone()).unwrap().plan.songs.len(), 1);
        let text = rides.export(saved.id.clone()).unwrap();
        assert!(text.contains("homefi_export"));
        assert!(rides.delete(saved.id).unwrap());
        assert!(rides.list().is_empty());
    }

    #[test]
    fn the_new_settings_cross_the_boundary_both_ways() {
        let custom = ExerciseSettings {
            bike: BikeProfile { console: ConsoleUnit::Kph, resistance_levels: 16, kph_per_rpm: 0.27 },
            climb_steps: 2,
            crossfade_seconds: 5.0,
            course_rider: CourseRider::Dot,
            ride_big_controls: true,
            ride_swap: false,
            swap_uses_ai: false,
            ..exercise_default_settings()
        };
        let core = ex::ExerciseSettings::from(custom.clone());
        assert_eq!((core.climb_steps, core.crossfade_seconds, core.course_rider), (2, 5.0, ex::CourseRider::Dot));
        assert_eq!((core.ride_big_controls, core.ride_swap, core.swap_uses_ai), (true, false, false));
        assert_eq!(core.bike.console, ex::ConsoleUnit::Kph);
        assert_eq!(ExerciseSettings::from(core), custom, "nothing is lost on the way");
        // the defaults are the core's, so a Kotlin copy of them keeps the product's choices
        let d = exercise_default_settings();
        assert_eq!((d.climb_steps, d.crossfade_seconds, d.ride_swap, d.swap_uses_ai), (3, 3.0, true, true));
        assert_eq!(d.bike, BikeProfile { console: ConsoleUnit::None, resistance_levels: 8, kph_per_rpm: 0.0 });
    }

    #[test]
    fn steps_and_the_swap_offset_survive_the_round_trip() {
        let segment = ex::Segment {
            kind: ex::SegmentKind::SeatedClimb,
            seconds: 300,
            steps: vec![
                ex::EffortStep { at_s: 100, effort: ex::Effort(7) },
                ex::EffortStep { at_s: 200, effort: ex::Effort(8) },
            ],
            ..ex::Segment::default()
        };
        let ffi = RideSegment::from(segment.clone());
        assert_eq!(ffi.steps, [EffortStep { at_s: 100, effort: 7 }, EffortStep { at_s: 200, effort: 8 }]);
        assert_eq!(ex::Segment::from(ffi), segment);
        let song = ex::PlannedSong { at_segment_ms: 60_000, play_ms: Some(1), ..ex::PlannedSong::default() };
        assert_eq!(ex::PlannedSong::from(PlannedSong::from(song.clone())), song);
    }

    #[test]
    fn the_bike_functions_are_the_cores() {
        let bike = BikeProfile { console: ConsoleUnit::Kph, resistance_levels: 8, kph_per_rpm: 0.25 };
        let shown = exercise_shown_target(bike, 70, 80);
        assert_eq!((shown.unit, shown.rpm, shown.value), (TargetUnit::Kph, 75, 18.8));
        assert_eq!(exercise_shown_target(BikeProfile { kph_per_rpm: 0.0, ..bike }, 70, 80).unit, TargetUnit::Rpm);
        assert_eq!(
            (exercise_bike_level(bike, 10), exercise_bike_level(BikeProfile { resistance_levels: 0, ..bike }, 5)),
            (Some(8), None)
        );
        assert_eq!(exercise_calibrate(ConsoleUnit::Kph, 80, 22.0), Some(0.275));
        assert_eq!(exercise_calibrate(ConsoleUnit::None, 80, 22.0), None);
        assert_eq!(exercise_calibration(), CalibrationSpec { rpm: 80, seconds: 20 });
    }

    #[test]
    fn a_climb_has_hot_points_and_the_view_carries_the_bike() {
        let settings = ex::ExerciseSettings::default();
        let climb = ex::Segment {
            kind: ex::SegmentKind::SeatedClimb,
            seconds: 300,
            target: ex::Target::Cadence { min_rpm: 60, max_rpm: 80 },
            effort: ex::Effort(7),
            ..ex::Segment::default()
        };
        let mut segments = vec![climb];
        ex::apply_climb_steps(&mut segments, settings.climb_steps);
        let workout = ex::Workout { segments, ..ex::Workout::default() };
        let points = exercise_hot_points(workout.clone().into());
        assert_eq!(points.iter().map(|p| (p.t_s, p.level, p.count)).collect::<Vec<_>>(), [(100, 2, 3), (200, 3, 3)]);
        let plan = ex::RidePlan {
            songs: vec![ex::PlannedSong::default()],
            profile: ex::profile(&workout),
            workout,
            ..ex::RidePlan::default()
        };
        let bike = BikeProfile { console: ConsoleUnit::Kph, resistance_levels: 8, kph_per_rpm: 0.25 };
        let ride = ExerciseRide::new(plan.clone().into(), Some(bike));
        let view = ride.view(0, 150_000, 0);
        assert_eq!(view.shown.unit, TargetUnit::Kph);
        assert_eq!((view.effort, view.level), (7, Some(6)), "the second level of the climb, as a knob position");
        assert_eq!(view.step.map(|s| (s.index, s.count)), Some((2, 3)));
        assert_eq!(ride.hot_points().len(), 2);
        // no bike given: RPM and no step of levels
        let plain = ExerciseRide::new(plan.into(), None).view(0, 150_000, 0);
        assert_eq!(plain.shown.unit, TargetUnit::Rpm);
    }

    #[test]
    fn defaults_and_clamping_are_the_cores() {
        let d = exercise_default_settings();
        assert_eq!((d.ride_minutes, d.tv_port, d.tv_audio), (30, 47801, TvAudio::Phone));
        assert_eq!(exercise_settings_clamped(d.clone()), d);
        let wild = exercise_settings_clamped(ExerciseSettings { ride_minutes: 500, cue_volume: 9.0, ..d });
        assert_eq!((wild.ride_minutes, wild.cue_volume), (90, 1.0));
    }

    #[test]
    fn settings_json_round_trips_and_garbage_gives_the_defaults() {
        let custom = ExerciseSettings { beginner_mode: false, tv_audio: TvAudio::Phone, ..exercise_default_settings() };
        assert_eq!(exercise_settings_from_json(exercise_settings_to_json(custom.clone())), custom);
        assert_eq!(exercise_settings_from_json("not json".into()), exercise_default_settings());
        assert_eq!(exercise_settings_from_json("{}".into()), exercise_default_settings());
        assert_eq!(exercise_settings_from_json(r#"{"tv_port": 3}"#.into()).tv_port, 1024);
    }

    #[test]
    fn presets_at_any_length_and_the_picker() {
        let settings = exercise_default_settings();
        for style in [PresetId::FirstRide, PresetId::Easy, PresetId::Classic, PresetId::Intervals] {
            let w = exercise_preset(style, 47, settings.clone());
            let total: u32 = w.segments.iter().map(|s| s.seconds).sum();
            assert!(total.abs_diff(45 * 60) <= 9 * 60, "{style:?}: {total}");
        }
        assert_eq!(exercise_ride_minutes().len(), 17);
        assert_eq!(exercise_presets("es".into())[1].name, "Paseo suave");
        assert_eq!(exercise_default_request(settings).minutes, 30);
    }

    fn plan() -> RidePlan {
        let workout = exercise_preset(PresetId::Easy, 20, exercise_default_settings());
        let songs = (0..workout.segments.len())
            .map(|i| PlannedSong {
                segment_index: i as u32,
                entry: PlaylistEntry::from(pl::PlaylistEntry { title: format!("S{i}"), ..Default::default() }),
                bpm: Some(90),
                source: Some(BpmSource::Tag),
                fit: None,
                grid: Some(DjGrid { first_beat_ms: 100.0, beat_ms: 666.7 }),
                pulse: PulseSource::SongBeat,
                play_ms: None,
                at_segment_ms: 0,
            })
            .collect();
        RidePlan {
            workout,
            songs,
            notes: Vec::new(),
            ai_problem: None,
            countdown_seconds: 3,
            notice_seconds: 10,
            wish: ex::MusicWish::default().into(),
            profile: Vec::new(),
        }
    }

    #[test]
    fn the_view_comes_from_the_core_clock() {
        let ride = ExerciseRide::new(plan(), None);
        let v = ride.view(0, 30_000, 5);
        assert_eq!((v.segment_index, v.kind, v.finished), (0, SegmentKind::WarmUp, false));
        assert_eq!(v.pulse.source, PulseSource::SongBeat);
        assert!(v.marker.elevation >= 0.0 && v.total_s > 0);
        assert_eq!(v, exercise_ride_view(plan(), 0, 30_000, 5, None));
        assert!(ride.view(99, 0, 0).finished);
        // the countdown of the first change is listed, later than now
        let near_end = u64::from(v.segment_seconds) * 1000 - 1_500;
        let end = ride.view(0, near_end, 1_000);
        assert!(end.cues.iter().any(|c| c.kind == CueKind::Countdown1));
        assert!(ride.schedule(0, near_end, 1_000).iter().all(|c| c.at_ms >= 1_000));
    }

    #[test]
    fn cues_are_wavs_and_the_listen_hub_is_listen_only() {
        for kind in [CueKind::Countdown3, CueKind::Finish, CueKind::Click] {
            let wav = exercise_cue_wav(kind, 44_100);
            assert_eq!(&wav[..4], b"RIFF");
            assert!(wav.len() > 100);
        }
        let p = exercise_listen_permissions();
        assert!(matches!(p.skip, crate::share::ShareSkipMode::Nobody) && !p.playback);
        assert_eq!(exercise_ride_token().len(), 8);
        assert_eq!(exercise_tv_protocol_version(), 1);
    }

    #[test]
    fn a_client_parses_ride_json() {
        let tv =
            exercise_tv_parse(r#"{"v":1,"server_now_ms":10,"title":"T","audio":"phone","extra":1}"#.into()).unwrap();
        assert_eq!((tv.v, tv.server_now_ms, tv.audio), (1, 10, TvAudio::Phone));
        assert!(tv.ride.is_none() && tv.listen.is_none());
        assert!(exercise_tv_parse(r#"{"v":9}"#.into()).is_err());
        assert!(exercise_tv_parse("nope".into()).is_err());
    }

    #[tokio::test]
    async fn the_tv_object_serves_while_running_and_publishes_the_phones_view() {
        let tv = ExerciseTv::new();
        let url = tv.clone().start(0, "Ride".into(), false, "en".into()).await.unwrap();
        assert!(url.starts_with("http://") && tv.is_running());
        let port = tv.port().unwrap();
        let ride = ExerciseRide::new(plan(), None);
        tv.begin(TvSession {
            title: "T".into(),
            profile: ride.plan.profile.iter().copied().map(Into::into).collect(),
            parts: ride.tv_parts(),
            audio: TvAudio::Phone,
            listen: None,
            pulse_style: PulseStyle::Ring,
            course_look: None,
            course_summits: false,
            hot_points: ride.hot_points(),
            course_rider: None,
        });
        tv.publish(ride, 0, 12_000, 1, true);
        let body = reqwest_get(port).await;
        let parsed = exercise_tv_parse(body).unwrap();
        assert!(parsed.playing && parsed.ride.is_some_and(|r| r.segment_index == 0));
        assert_eq!(parsed.text.segment, "Warm-up");
        tv.clone().stop().await;
        assert!(!tv.is_running());
    }

    async fn reqwest_get(port: u16) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        stream.write_all(b"GET /ride.json HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").await.unwrap();
        let mut raw = String::new();
        stream.read_to_string(&mut raw).await.unwrap();
        raw.split("\r\n\r\n").nth(1).unwrap_or_default().to_owned()
    }
}
