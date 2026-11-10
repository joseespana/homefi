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
}
mirror_enum!(CueKind <=> ex::CueKind {
    Countdown3, Countdown2, Countdown1, Change, Standing, Seated, SprintGo, Recover, Finish, Click
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct RideNext {
    pub kind: SegmentKind,
    pub position: RidePosition,
    pub target_rpm: u16,
    pub seconds: u32,
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
    pub effort: u8,
    /// The big number.
    pub target_rpm: u16,
    pub min_rpm: u16,
    pub max_rpm: u16,
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
}

#[uniffi::export]
impl ExerciseRide {
    #[uniffi::constructor]
    pub fn new(plan: RidePlan) -> Arc<Self> {
        Arc::new(Self { plan: Arc::new(plan.into()) })
    }

    /// What the screen shows at `queue_index` (the playing song of the ride's queue) and `position_ms`
    /// (its position). Pure: a paused player gives the same view.
    pub fn view(&self, queue_index: u32, position_ms: u64, now_ms: u64) -> RideView {
        ex::RideClock::view(&self.plan, queue_index as usize, position_ms, now_ms).into()
    }

    /// The parts of the ride for [`TvSession::parts`] (the mountain's colours).
    pub fn tv_parts(&self) -> Vec<TvPart> {
        ex::TvPart::of(&self.plan.workout).into_iter().map(Into::into).collect()
    }

    /// The cues of the view still to come, as `(at_ms, kind)` on the view's clock.
    pub fn schedule(&self, queue_index: u32, position_ms: u64, now_ms: u64) -> Vec<CueAt> {
        let view = ex::RideClock::view(&self.plan, queue_index as usize, position_ms, now_ms);
        ex::schedule(&view).into_iter().map(|(at_ms, kind)| CueAt { at_ms, kind: kind.into() }).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct CueAt {
    pub at_ms: u64,
    pub kind: CueKind,
}

/// [`ExerciseRide::view`] without keeping the plan (converts it every call: prefer the object).
#[uniffi::export]
pub fn exercise_ride_view(plan: RidePlan, queue_index: u32, position_ms: u64, now_ms: u64) -> RideView {
    ex::RideClock::view(&ex::RidePlan::from(plan), queue_index as usize, position_ms, now_ms).into()
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

#[uniffi::export]
impl HomefiEngine {
    /// Builds the ride (not saved: the app saves `playlist` when the ride starts). The AI's key travels apart
    /// and is never stored. Cancelling the coroutine stops the work between songs. Building the same request
    /// again ("new songs") reuses what the last build found and plays songs it did not.
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
            // signed out is not an error: only the library and the AI are used
            let _ = self.session.refresh(600, false).await;
            let api = self.session.api().ok();
            let ai = ai.map(|c| core_config(c, api_key));
            let index = library_index(library).await;
            let settings = ex::ExerciseSettings::from(settings);
            let analyzer = FileAnalyzer {
                dir: ANALYSIS_DIR.iter().fold(self.session.app_dir().to_path_buf(), |p, part| p.join(part)),
                memory_mb: homefi_core::dj::settings::ANDROID_MEMORY_MB,
            };
            let ctx = ex::BuildContext {
                settings: &settings,
                ai: ai.as_ref(),
                api: api.as_ref(),
                library: &index,
                analyzer: Some(&analyzer),
                lang: Lang::from_code(&language),
            };
            // the last build's memory: "new songs" for the same ride reuses what it found
            let memory = self.ride_memory();
            let (built, memory) =
                ex::build_with_memory(&request.into(), &ctx, &|p| listener.on_progress(p.into()), memory).await?;
            self.remember_ride(memory);
            Ok(RideBuilt { plan: built.plan.into(), playlist: built.playlist.into() })
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
    pub effort: String,
    pub next: String,
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
            },
            pulse_style: v.pulse_style.into(),
            course_look: v.course_look.into(),
            course_summits: v.course_summits,
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
            });
        }
    }

    /// Publishes the ride's state as the phone shows it (the same clock), with `playing` false when paused.
    pub fn publish(&self, ride: Arc<ExerciseRide>, queue_index: u32, position_ms: u64, now_ms: u64, playing: bool) {
        if let Some(server) = lock(&self.server).as_ref() {
            server.publish(ex::RideClock::view(&ride.plan, queue_index as usize, position_ms, now_ms), playing);
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
            })
            .collect();
        RidePlan {
            workout,
            songs,
            notes: Vec::new(),
            countdown_seconds: 3,
            notice_seconds: 10,
            wish: ex::MusicWish::default().into(),
            profile: Vec::new(),
        }
    }

    #[test]
    fn the_view_comes_from_the_core_clock() {
        let ride = ExerciseRide::new(plan());
        let v = ride.view(0, 30_000, 5);
        assert_eq!((v.segment_index, v.kind, v.finished), (0, SegmentKind::WarmUp, false));
        assert_eq!(v.pulse.source, PulseSource::SongBeat);
        assert!(v.marker.elevation >= 0.0 && v.total_s > 0);
        assert_eq!(v, exercise_ride_view(plan(), 0, 30_000, 5));
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
        let ride = ExerciseRide::new(plan());
        tv.begin(TvSession {
            title: "T".into(),
            profile: ride.plan.profile.iter().copied().map(Into::into).collect(),
            parts: ride.tv_parts(),
            audio: TvAudio::Phone,
            listen: None,
            pulse_style: PulseStyle::Ring,
            course_look: None,
            course_summits: false,
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
