//! The workout: what a ride asks of the rider, segment by segment. Read leniently (missing fields
//! take their defaults, unknown fields are ignored, unknown words read as the default) so a file or an
//! AI answer from another version still loads.

use serde::{Deserialize, Serialize};

use crate::ai::prompt::one_line;

pub const WORKOUT_FORMAT_VERSION: u32 = 1;
/// Ride length the rider may choose (minutes), in steps of [`RIDE_MINUTES_STEP`].
pub const MIN_RIDE_MINUTES: u16 = 10;
pub const MAX_RIDE_MINUTES: u16 = 90;
pub const RIDE_MINUTES_STEP: u16 = 5;
/// Cadence limits (RPM) of any target.
pub const MIN_RPM: u16 = 30;
pub const MAX_RPM: u16 = 140;
pub const MIN_EFFORT: u8 = 1;
pub const MAX_EFFORT: u8 = 10;
/// Segment length limits (s).
pub const MIN_SEGMENT_SECONDS: u32 = 20;
pub const MAX_SEGMENT_SECONDS: u32 = 20 * 60;
/// Most segments of one workout.
pub const MAX_SEGMENTS: usize = 40;
pub const MAX_ROUNDS: u8 = 12;
/// Resistance steps inside one segment (a climb "asks for one more turn"), and the shortest a step may last (s).
pub const MAX_STEPS: usize = 4;
pub const MIN_STEP_SECONDS: u32 = 40;
/// Genres the wish keeps, and the longest genre or sentence (chars).
pub const MAX_GENRES: usize = 6;
pub const MAX_GENRE_CHARS: usize = 40;
pub const MAX_SENTENCE_CHARS: usize = 300;
pub const MAX_NAME_CHARS: usize = 80;
pub const MAX_NOTE_CHARS: usize = 200;
const BEATS_PER_BAR: u8 = 8;

/// Rounds a length to the nearest 5 minutes inside the allowed range.
pub fn round_minutes(minutes: u16) -> u16 {
    let steps = (minutes + RIDE_MINUTES_STEP / 2) / RIDE_MINUTES_STEP;
    (steps * RIDE_MINUTES_STEP).clamp(MIN_RIDE_MINUTES, MAX_RIDE_MINUTES)
}

/// Other sports (weights...) are later `Sport` and `Target` variants on the same model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Sport {
    #[default]
    #[serde(other)]
    IndoorCycling,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SegmentKind {
    WarmUp,
    SeatedClimb,
    StandingClimb,
    Jumps,
    Sprint,
    Recovery,
    CoolDown,
    #[default]
    #[serde(other)]
    SeatedFlat,
}

/// A cadence range (revolutions per minute).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RpmRange {
    pub min_rpm: u16,
    pub max_rpm: u16,
}

impl Default for RpmRange {
    fn default() -> Self {
        Self { min_rpm: 80, max_rpm: 95 }
    }
}

impl RpmRange {
    pub fn new(min_rpm: u16, max_rpm: u16) -> Self {
        Self { min_rpm, max_rpm }.clamped()
    }

    /// Inside [`MIN_RPM`]..=[`MAX_RPM`] and ordered.
    pub fn clamped(self) -> Self {
        let a = self.min_rpm.clamp(MIN_RPM, MAX_RPM);
        let b = self.max_rpm.clamp(MIN_RPM, MAX_RPM);
        Self { min_rpm: a.min(b), max_rpm: a.max(b) }
    }

    /// The number shown big.
    pub fn center(self) -> u16 {
        ((u32::from(self.min_rpm) + u32::from(self.max_rpm)) / 2) as u16
    }

    /// How far `rpm` is outside the range (0 inside).
    pub fn distance(self, rpm: f32) -> f32 {
        (f32::from(self.min_rpm) - rpm).max(rpm - f32::from(self.max_rpm)).max(0.0)
    }
}

/// What a segment asks for. Tagged so weights can add `Reps` without a migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Target {
    Cadence { min_rpm: u16, max_rpm: u16 },
}

impl Default for Target {
    fn default() -> Self {
        Self::Cadence { min_rpm: 80, max_rpm: 95 }
    }
}

impl Target {
    pub fn cadence(self) -> RpmRange {
        let Self::Cadence { min_rpm, max_rpm } = self;
        RpmRange { min_rpm, max_rpm }.clamped()
    }
}

impl From<RpmRange> for Target {
    fn from(r: RpmRange) -> Self {
        Self::Cadence { min_rpm: r.min_rpm, max_rpm: r.max_rpm }
    }
}

/// Perceived effort, 1 (very easy) to 10 (all out): a resistance cue, not a measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Effort(pub u8);

impl Default for Effort {
    fn default() -> Self {
        Self(5)
    }
}

impl Effort {
    pub fn get(self) -> u8 {
        self.0.clamp(MIN_EFFORT, MAX_EFFORT)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Position {
    #[default]
    Seated,
    Standing,
    /// Up and down every `beats` beats (jumps).
    Alternating {
        beats: u8,
    },
}

impl Position {
    /// Standing at least part of the time.
    pub fn is_standing(self) -> bool {
        !matches!(self, Self::Seated)
    }

    /// Alternating every [`BEATS_PER_BAR`] beats, one bar.
    pub fn jumps() -> Self {
        Self::Alternating { beats: BEATS_PER_BAR }
    }
}

/// Work and rest inside one segment (sprints), repeated `rounds` times, work first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Interval {
    pub work_s: u32,
    pub rest_s: u32,
    pub rounds: u8,
    /// Cadence in the rest phases (the segment's target is the work cadence).
    pub rest_cadence: RpmRange,
}

impl Default for Interval {
    fn default() -> Self {
        Self { work_s: 20, rest_s: 60, rounds: 4, rest_cadence: RpmRange { min_rpm: 80, max_rpm: 90 } }
    }
}

impl Interval {
    pub fn cycle_s(&self) -> u32 {
        self.work_s + self.rest_s
    }

    /// Time the rounds take.
    pub fn total_s(&self) -> u32 {
        self.cycle_s() * u32::from(self.rounds)
    }
}

/// From `at_s` seconds into its segment, the effort is `effort` (a climb's "one more turn of resistance").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EffortStep {
    pub at_s: u32,
    pub effort: Effort,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Segment {
    pub kind: SegmentKind,
    pub seconds: u32,
    pub target: Target,
    pub effort: Effort,
    pub position: Position,
    pub interval: Option<Interval>,
    pub note: String,
    /// Effort changes inside the segment, ordered by `at_s` (`0 < at_s < seconds`, at most [`MAX_STEPS`]).
    /// Before the first one the effort is `effort`. Left out of the JSON when empty, so old files read the same.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<EffortStep>,
}

impl Default for Segment {
    fn default() -> Self {
        Self {
            kind: SegmentKind::default(),
            seconds: 240,
            target: Target::default(),
            effort: Effort::default(),
            position: Position::default(),
            interval: None,
            note: String::new(),
            steps: Vec::new(),
        }
    }
}

impl Segment {
    pub fn cadence(&self) -> RpmRange {
        self.target.cadence()
    }

    /// The effort `position_s` seconds into the segment: the last step that has started, else the base effort.
    pub fn effort_at(&self, position_s: u32) -> u8 {
        self.steps.iter().rev().find(|s| s.at_s <= position_s).map_or(self.effort, |s| s.effort).get()
    }

    /// Everything inside its range; an interval fits in the segment (rounds are lowered, then the
    /// segment grows to hold at least one).
    pub fn clamped(mut self) -> Self {
        self.target = self.cadence().into();
        self.effort = Effort(self.effort.get());
        self.seconds = self.seconds.clamp(MIN_SEGMENT_SECONDS, MAX_SEGMENT_SECONDS);
        if let Position::Alternating { beats } = self.position {
            self.position = Position::Alternating { beats: beats.clamp(2, 32) };
        }
        self.note = self.note.chars().take(MAX_NOTE_CHARS).collect();
        self.steps.retain(|s| s.at_s > 0 && s.at_s < self.seconds);
        self.steps.sort_by_key(|s| s.at_s);
        self.steps.dedup_by_key(|s| s.at_s);
        self.steps.truncate(MAX_STEPS);
        for step in &mut self.steps {
            step.effort = Effort(step.effort.get());
        }
        if let Some(mut iv) = self.interval {
            iv.work_s = iv.work_s.clamp(5, MAX_SEGMENT_SECONDS);
            iv.rest_s = iv.rest_s.clamp(5, MAX_SEGMENT_SECONDS);
            iv.rest_cadence = iv.rest_cadence.clamped();
            iv.rounds = iv.rounds.clamp(1, MAX_ROUNDS);
            let fits = (self.seconds / iv.cycle_s()).clamp(1, u32::from(iv.rounds));
            iv.rounds = fits as u8;
            self.seconds = self.seconds.max(iv.cycle_s());
            self.interval = Some(iv);
        }
        self
    }
}

/// Where a workout came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkoutOrigin {
    Preset {
        preset: PresetId,
    },
    Ai {
        wish: String,
        provider_id: String,
        model: String,
    },
    #[default]
    #[serde(other)]
    Custom,
}

/// The ride styles. Each works at any length (see `presets::preset`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PresetId {
    /// Adds jumps and one short sprint block per round.
    Easy,
    /// Standing and seated climbs and longer sprint blocks.
    Classic,
    /// Mostly sprint blocks with recoveries.
    Intervals,
    /// Warm-up, flat, a seated climb, recovery and cool-down: no jumps, no sprints.
    #[default]
    #[serde(other)]
    FirstRide,
}

impl PresetId {
    pub const ALL: [PresetId; 4] = [Self::FirstRide, Self::Easy, Self::Classic, Self::Intervals];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Workout {
    pub version: u32,
    /// `[A-Za-z0-9_-]{1,64}`, like a playlist id.
    pub id: String,
    pub name: String,
    pub sport: Sport,
    pub segments: Vec<Segment>,
    pub origin: WorkoutOrigin,
}

impl Default for Workout {
    fn default() -> Self {
        Self {
            version: WORKOUT_FORMAT_VERSION,
            id: String::new(),
            name: String::new(),
            sport: Sport::default(),
            segments: Vec::new(),
            origin: WorkoutOrigin::default(),
        }
    }
}

impl Workout {
    pub fn total_seconds(&self) -> u32 {
        self.segments.iter().map(|s| s.seconds).sum()
    }

    /// Segments inside their ranges and at most [`MAX_SEGMENTS`]; a name cut to length. For anything
    /// that did not come from our own presets (the AI's answer, a custom editor).
    pub fn sanitized(mut self) -> Self {
        self.segments.truncate(MAX_SEGMENTS);
        self.segments = self.segments.into_iter().map(Segment::clamped).collect();
        self.name = self.name.trim().chars().take(MAX_NAME_CHARS).collect();
        self
    }
}

/// How famous the songs of a ride should be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Popularity {
    /// Some hits, some lesser-known songs by established acts.
    Mix,
    /// Lesser-known songs and artists.
    Discover,
    /// The genre's big hits by its famous acts: songs the rider knows and sings along to (the default).
    #[default]
    #[serde(other)]
    Hits,
}

impl Popularity {
    pub const ALL: [Popularity; 3] = [Self::Hits, Self::Mix, Self::Discover];
}

/// Decades the rider may pick (the year a decade starts), oldest first; none picked means any era.
pub const DECADES: [u16; 7] = [1960, 1970, 1980, 1990, 2000, 2010, 2020];
/// Artists the rider likes (or wants to avoid) that a taste keeps, and the longest name (chars).
pub const MAX_TASTE_ARTISTS: usize = 10;
pub const MAX_ARTIST_CHARS: usize = 60;

/// The rider's taste for a ride: when the music is from, how famous, who they like and who never to play.
/// Remembered in the settings so the next ride starts from the same answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RideTaste {
    /// Decades (see [`DECADES`]); empty = any era.
    pub eras: Vec<u16>,
    pub popularity: Popularity,
    /// Artists the rider likes: they lead the AI's picks.
    pub artists: Vec<String>,
    /// Artists never to play.
    pub avoid: Vec<String>,
}

/// Names on one line, without empties or repeats (any case), at most [`MAX_TASTE_ARTISTS`] of [`MAX_ARTIST_CHARS`].
fn clean_names(names: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in names {
        let name = one_line(&name, MAX_ARTIST_CHARS);
        if !name.is_empty() && !out.iter().any(|x| x.to_lowercase() == name.to_lowercase()) {
            out.push(name);
        }
    }
    out.truncate(MAX_TASTE_ARTISTS);
    out
}

impl RideTaste {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Only known decades, oldest first and once; artist lists cleaned.
    pub fn clamped(self) -> Self {
        let eras = DECADES.iter().copied().filter(|d| self.eras.contains(d)).collect();
        Self { eras, popularity: self.popularity, artists: clean_names(self.artists), avoid: clean_names(self.avoid) }
    }
}

/// What the rider wants to hear: genres (one, or several for a mix), a free sentence and their taste.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MusicWish {
    /// "rock", "metal"... at most [`MAX_GENRES`].
    pub genres: Vec<String>,
    /// Several genres spread over the segments by tempo (metal for sprints, calmer for the warm-up).
    pub mix: bool,
    pub sentence: String,
    /// Left out of the JSON when it is the default, so older files and playlists read and write the same.
    #[serde(skip_serializing_if = "RideTaste::is_default")]
    pub taste: RideTaste,
}

impl MusicWish {
    /// Genres trimmed, without empties or repeats (any case), short; the sentence cut to length.
    pub fn clamped(self) -> Self {
        let mut genres: Vec<String> = Vec::new();
        for g in self.genres {
            let g = one_line(&g, MAX_GENRE_CHARS);
            if !g.is_empty() && !genres.iter().any(|x| x.eq_ignore_ascii_case(&g)) {
                genres.push(g);
            }
        }
        genres.truncate(MAX_GENRES);
        Self {
            genres,
            mix: self.mix,
            sentence: one_line(&self.sentence, MAX_SENTENCE_CHARS),
            taste: self.taste.clamped(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.genres.is_empty() && self.sentence.trim().is_empty() && self.taste.artists.is_empty()
    }
}

/// What the rider asks the builder for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RideRequest {
    /// Any length from 10 to 90, rounded to 5.
    pub minutes: u16,
    pub style: PresetId,
    pub wish: MusicWish,
    /// A workout of the rider's own or the AI's; `None` builds `style` at `minutes`.
    pub workout: Option<Workout>,
}

impl Default for RideRequest {
    fn default() -> Self {
        Self { minutes: 30, style: PresetId::default(), wish: MusicWish::default(), workout: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minutes_round_to_five_inside_the_range() {
        assert_eq!(round_minutes(0), 10);
        assert_eq!(round_minutes(12), 10);
        assert_eq!(round_minutes(13), 15);
        assert_eq!(round_minutes(47), 45);
        assert_eq!(round_minutes(90), 90);
        assert_eq!(round_minutes(500), 90);
    }

    #[test]
    fn rpm_ranges_order_clamp_and_measure() {
        let r = RpmRange::new(120, 20);
        assert_eq!((r.min_rpm, r.max_rpm), (30, 120));
        let r = RpmRange::new(80, 90);
        assert_eq!(r.center(), 85);
        assert_eq!((r.distance(85.0), r.distance(75.0), r.distance(93.0)), (0.0, 5.0, 3.0));
    }

    #[test]
    fn a_lenient_workout_reads_and_unknown_words_take_defaults() {
        let json = r#"{"name":"X","future":1,"segments":[
            {"kind":"moonwalk","seconds":300,"target":{"kind":"cadence","min_rpm":70,"max_rpm":60},"effort":15,
             "position":{"kind":"alternating","beats":8}}, {}],"origin":{"kind":"from_space"}}"#;
        let w: Workout = serde_json::from_str(json).unwrap();
        assert_eq!(w.segments[0].kind, SegmentKind::SeatedFlat);
        assert_eq!(w.origin, WorkoutOrigin::Custom);
        let w = w.sanitized();
        assert_eq!(w.segments[0].cadence(), RpmRange { min_rpm: 60, max_rpm: 70 });
        assert_eq!(w.segments[0].effort, Effort(10));
        assert_eq!(w.segments[1], Segment::default().clamped());
        let back: Workout = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        assert_eq!(back, w);
    }

    #[test]
    fn an_interval_always_fits_its_segment() {
        let seg = Segment {
            seconds: 100,
            interval: Some(Interval { work_s: 30, rest_s: 60, rounds: 6, ..Interval::default() }),
            ..Segment::default()
        }
        .clamped();
        let iv = seg.interval.unwrap();
        assert_eq!(iv.rounds, 1);
        assert!(iv.total_s() <= seg.seconds);
        let tiny = Segment { seconds: 20, interval: Some(Interval::default()), ..Segment::default() }.clamped();
        assert!(tiny.seconds >= tiny.interval.unwrap().cycle_s());
    }

    #[test]
    fn the_wish_is_cleaned() {
        let wish = MusicWish {
            genres: vec![" Rock ".into(), "rock".into(), "".into(), "Metal".into()],
            mix: true,
            sentence: "  90s  ".into(),
            ..MusicWish::default()
        }
        .clamped();
        assert_eq!(wish.genres, ["Rock", "Metal"]);
        assert_eq!(wish.sentence, "90s");
        assert!(MusicWish::default().is_empty());
    }

    #[test]
    fn the_taste_is_cleaned_and_reads_leniently() {
        let taste = RideTaste {
            eras: vec![2020, 1985, 1980, 1980, 1950],
            popularity: Popularity::Mix,
            artists: vec![" Metallica ".into(), "metallica".into(), "".into(), "x".repeat(200)],
            avoid: (0..30).map(|i| format!("Band {i}")).collect(),
        }
        .clamped();
        assert_eq!(taste.eras, [1980, 2020], "known decades, oldest first, once");
        assert_eq!(taste.artists.len(), 2);
        assert_eq!(taste.artists[0], "Metallica");
        assert_eq!(taste.artists[1].chars().count(), MAX_ARTIST_CHARS);
        assert_eq!(taste.avoid.len(), MAX_TASTE_ARTISTS);
        // an older wish without a taste, or an unknown popularity word, reads as big hits
        let old: MusicWish = serde_json::from_str(r#"{"genres":["metal"],"mix":false,"sentence":""}"#).unwrap();
        assert_eq!(old.taste, RideTaste::default());
        assert_eq!(old.taste.popularity, Popularity::Hits);
        let odd: RideTaste = serde_json::from_str(r#"{"popularity":"viral","eras":[1990]}"#).unwrap();
        assert_eq!((odd.popularity, odd.eras.as_slice()), (Popularity::Hits, [1990].as_slice()));
        assert_eq!(serde_json::to_string(&Popularity::Discover).unwrap(), "\"discover\"");
    }

    #[test]
    fn effort_at_follows_the_steps() {
        let seg = Segment {
            seconds: 300,
            effort: Effort(6),
            steps: vec![EffortStep { at_s: 100, effort: Effort(7) }, EffortStep { at_s: 200, effort: Effort(8) }],
            ..Segment::default()
        };
        assert_eq!([0, 99, 100, 199, 200, 299].map(|t| seg.effort_at(t)), [6, 6, 7, 7, 8, 8]);
        assert_eq!(Segment { effort: Effort(40), ..Segment::default() }.effort_at(5), MAX_EFFORT);
    }

    #[test]
    fn steps_are_ordered_unique_inside_the_segment_and_capped() {
        let step = |at_s, effort| EffortStep { at_s, effort: Effort(effort) };
        let seg = Segment {
            seconds: 300,
            steps: vec![
                step(200, 9),
                step(0, 5),
                step(100, 99),
                step(100, 3),
                step(300, 7),
                step(10, 6),
                step(20, 6),
                step(30, 6),
            ],
            ..Segment::default()
        }
        .clamped();
        let at: Vec<u32> = seg.steps.iter().map(|s| s.at_s).collect();
        assert_eq!(at.len(), MAX_STEPS);
        assert!(at.windows(2).all(|w| w[0] < w[1]) && at.iter().all(|&t| t > 0 && t < 300));
        assert!(seg.steps.iter().all(|s| s.effort.get() <= MAX_EFFORT));
    }

    #[test]
    fn a_segment_without_steps_reads() {
        let old: Segment = serde_json::from_str(r#"{"kind":"seated_climb","seconds":240,"effort":7}"#).unwrap();
        assert!(old.steps.is_empty());
        assert!(!serde_json::to_string(&old).unwrap().contains("steps"), "no steps, nothing written");
        let with: Segment =
            serde_json::from_str(r#"{"seconds":240,"steps":[{"at_s":80,"effort":8},{"future":1}]}"#).unwrap();
        assert_eq!(with.steps.len(), 2);
        let with = with.clamped();
        assert_eq!(with.steps.len(), 1, "the step at 0 is dropped");
        assert_eq!(with.effort_at(100), 8);
        let back: Segment = serde_json::from_str(&serde_json::to_string(&with).unwrap()).unwrap();
        assert_eq!(back, with);
    }
}
