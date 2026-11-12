//! Exercise mode: indoor cycling led by the music. A ride is a list of segments (warm-up, climbs,
//! sprints...), each with a target cadence; HomeFi picks songs whose tempo makes "pedal on the beat"
//! true and the player plays them as a normal HomeFi playlist. Pure and Android-safe: no audio device,
//! no screen. Design: `docs/EXERCISE_PLAN.md`.
//!
//! - [`model`]: the workout, its segments and the request.
//! - [`presets`]: the styles (any length) and the beginner caps.
//! - [`settings`]: every option, with defaults and ranges in one place; [`taste`]: artists to suggest.
//! - [`bike`]: the rider's bike: the console's unit, resistance levels, speed calibration.
//! - [`bpm`]: does a song's tempo fit a cadence? (1:1, half-time, double-time) and the widening ladder.
//! - [`builder`]: request to ride plan and playlist; [`ai`]: the chat and the song proposals.
//! - [`swap`]: another song for one part of a built ride, from what the build found or a new search.
//! - [`library`]: saved rides (auto-saved builds, kept, favorites) and their export / import.
//! - [`ride`]: what the screen shows at a queue index and song position; [`cues`]: the sounds.
//! - [`tv`]: the JSON contract of the TV page and (feature `share`) its server and discovery.

pub mod ai;
pub mod bike;
pub mod bpm;
pub mod builder;
pub mod cues;
pub mod library;
pub mod model;
pub mod presets;
pub mod profile;
pub mod ride;
pub mod settings;
pub mod swap;
pub mod taste;
pub mod tv;
pub(crate) mod words;

pub use ai::{ExerciseSession, ExerciseTurn};
pub use bike::{BikeProfile, ConsoleUnit, ShownTarget, TargetUnit};
pub use bpm::{fit, ladder, BeatRatio, BpmSource, Fit, Rung, Tolerance};
pub use builder::{
    build, build_with_memory, Analyzer, BuildContext, BuildMemory, CueSettings, PlannedSong, PulseSource, RideBuilt,
    RidePlan, RideProgress, BUILD_DEADLINE,
};
pub use cues::{schedule, CueKind};
pub use library::{auto_name, export_ride, import_ride, RideStore, RideSummary, SavedRide, SAVED_RIDE_FORMAT_VERSION};
pub use model::{
    round_minutes, Effort, EffortStep, Interval, MusicWish, Popularity, Position, PresetId, RideRequest, RideTaste,
    RpmRange, Segment, SegmentKind, Sport, Target, Workout, WorkoutOrigin, DECADES, MAX_ARTIST_CHARS, MAX_RIDE_MINUTES,
    MAX_STEPS, MAX_TASTE_ARTISTS, MIN_RIDE_MINUTES, MIN_STEP_SECONDS, RIDE_MINUTES_STEP,
};
pub use presets::{apply_climb_steps, beginner_caps, climb_steps, preset};
pub use profile::{elevation_at, hot_points, profile, HotPoint, ProfilePoint};
pub use ride::{Cue, IntervalPhase, IntervalView, NextView, PulseView, RideClock, RideView, StepView};
pub use settings::{
    lenient_exercise, CourseLook, CourseRider, ExerciseSettings, PulseStyle, SourcesMode, TvAudio, DEFAULT_TV_PORT,
    TV_DISPLAY_AUTO,
};
pub use swap::{swap_song, SwapRequest, Swapped};
pub use taste::{artist_suggestions, MAX_ARTIST_SUGGESTIONS};
#[cfg(feature = "share")]
pub use tv::{discover, RideTvEndpoint, TvServer, RIDE_SERVICE_TYPE};
pub use tv::{render_page, RideTvView, TvPart, TvText, TV_PROTOCOL_VERSION};

use crate::humane::Lang;

/// The short "not medical advice" notice shown once before the first ride, in `lang`.
pub fn health_notice(lang: Lang) -> String {
    words::health_notice(lang)
}

/// "Describing your ride in words needs an AI..." in `lang`.
pub fn needs_ai_message(lang: Lang) -> String {
    words::needs_ai(lang)
}

/// The name of a style ("Easy ride") in `lang`.
pub fn style_name(lang: Lang, style: PresetId) -> String {
    words::style_name(lang, style).to_owned()
}
