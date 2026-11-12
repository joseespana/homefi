//! What the ride screen shows right now. Pure: the segment follows the queue index, the interval phases
//! follow the song's position, and a paused player (a position that does not move) freezes everything.

use serde::{Deserialize, Serialize};

use super::bike::{BikeProfile, ShownTarget, MAX_RESISTANCE_LEVELS};
use super::bpm::{nearest_ratio, BeatRatio};
use super::builder::{PlannedSong, PulseSource, RidePlan};
use super::cues::CueKind;
use super::model::{Interval, Position, RpmRange, Segment, SegmentKind};
use super::profile::{elevation_at, profile, ProfilePoint};

const MS_PER_S: u64 = 1000;
const MS_PER_MINUTE: f64 = 60_000.0;
/// Cues further ahead than this are not listed yet.
pub const CUE_HORIZON_MS: i64 = 15_000;
/// A cue this far in the past is still listed (a view that arrived a little late).
pub const CUE_LATE_MS: i64 = 500;
/// A countdown only starts inside a phase at least this much longer than itself (s).
const MIN_PHASE_MARGIN_S: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IntervalPhase {
    Rest,
    #[default]
    #[serde(other)]
    Work,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct IntervalView {
    pub phase: IntervalPhase,
    /// 1-based.
    pub round: u8,
    pub rounds: u8,
    pub phase_remaining_s: u32,
}

/// The next segment, for "next: standing climb · 65 RPM".
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct NextView {
    pub kind: SegmentKind,
    pub position: Position,
    pub target_rpm: u16,
    /// The target in the bike's unit (see [`BikeProfile::target`]).
    pub shown: ShownTarget,
    /// The knob position of the next segment's starting effort; `None` when the bike's levels are unknown.
    pub level: Option<u8>,
    pub seconds: u32,
}

/// Where the rider is inside a climb's resistance steps: "level 2 of 3".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct StepView {
    /// 1-based; the climb's own start is 1, so after the first step it is 2.
    pub index: u8,
    pub count: u8,
    /// The effort at this level.
    pub effort: u8,
    /// Seconds to the next step, `None` on the last level.
    pub next_in_s: Option<u32>,
}

/// The beat the rider pedals to, computed from the song's grid (or a metronome).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PulseView {
    pub source: PulseSource,
    pub first_beat_ms: f64,
    pub beat_ms: f64,
    /// Time between pedal strokes: `beat_ms` divided by the ratio's strokes per beat.
    pub stroke_ms: f64,
    pub ratio: BeatRatio,
    /// The song's position this view was made at.
    pub song_position_ms: u64,
}

impl Default for PulseView {
    fn default() -> Self {
        Self {
            source: PulseSource::Metronome,
            first_beat_ms: 0.0,
            beat_ms: 0.0,
            stroke_ms: 0.0,
            ratio: BeatRatio::Whole,
            song_position_ms: 0,
        }
    }
}

impl PulseView {
    /// Where `position_ms` of the song is inside its pedal stroke, 0..1 (0 = on the stroke).
    pub fn stroke_phase(&self, position_ms: f64) -> f64 {
        if self.stroke_ms <= 0.0 {
            return 0.0;
        }
        ((position_ms - self.first_beat_ms) / self.stroke_ms).rem_euclid(1.0)
    }

    /// Milliseconds from the view's position to the next pedal stroke.
    pub fn next_stroke_in_ms(&self) -> f64 {
        if self.stroke_ms <= 0.0 {
            return 0.0;
        }
        (1.0 - self.stroke_phase(self.song_position_ms as f64)) % 1.0 * self.stroke_ms
    }
}

/// A sound to play `in_ms` from the view's moment (negative: just passed). `id` is stable across views,
/// so a screen that sees the same cue twice plays it once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cue {
    pub id: u32,
    pub kind: CueKind,
    pub in_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RideView {
    /// The moment this view describes (ms, the caller's clock).
    pub at_ms: u64,
    /// The last song is over.
    pub finished: bool,
    pub segment_index: u32,
    pub segment_count: u32,
    pub kind: SegmentKind,
    pub position: Position,
    /// The effort now (`Segment::effort_at`: a climb's steps raise it).
    pub effort: u8,
    /// The big number: the center of the current phase's range.
    pub target_rpm: u16,
    pub range: RpmRange,
    /// The range in the bike's unit (km/h once calibrated); `shown.rpm` is `target_rpm`.
    pub shown: ShownTarget,
    /// The bike's knob position for `effort`; `None` when its levels are unknown.
    pub level: Option<u8>,
    /// How many positions the knob has (0 when `level` is `None`), for "6/8".
    pub level_count: u8,
    /// The resistance step the rider is on; `None` for a segment without steps.
    pub step: Option<StepView>,
    pub segment_seconds: u32,
    /// Seconds from the start of the ride to the start of this segment (highlights it on the profile).
    pub segment_start_s: u32,
    pub remaining_s: u32,
    /// 0..=1 through the segment.
    pub segment_progress: f32,
    pub elapsed_s: u32,
    pub total_s: u32,
    /// 0..=1 through the ride.
    pub progress: f32,
    /// Where the rider is on the course profile (`RidePlan::profile`).
    pub marker: ProfilePoint,
    pub interval: Option<IntervalView>,
    /// The change is closer than the plan's notice time: show "next".
    pub notice: bool,
    pub next: Option<NextView>,
    pub pulse: PulseView,
    pub cues: Vec<Cue>,
    /// The playing song has reached its cut (`PlannedSong::play_ms`): the app must go to the next song now
    /// (on the last one, end the ride). Stays true until the queue moves.
    pub skip_to_next: bool,
}

/// Namespace of the pure view function.
pub struct RideClock;

impl RideClock {
    /// The view at `queue_index` (the playing song of the ride's queue) and `position_ms` (its position).
    pub fn view(plan: &RidePlan, queue_index: usize, position_ms: u64, now_ms: u64) -> RideView {
        Self::view_on(plan, &BikeProfile::default(), queue_index, position_ms, now_ms)
    }

    /// [`Self::view`] for a rider's bike: targets in its console's unit, efforts as knob positions. A song
    /// swapped in mid-segment (`PlannedSong::at_segment_ms`) keeps the segment's clock going: the timer, the
    /// effort, the interval phases and the cues follow the segment's time, the pulse the song's position.
    pub fn view_on(plan: &RidePlan, bike: &BikeProfile, queue_index: usize, position_ms: u64, now_ms: u64) -> RideView {
        let segments = &plan.workout.segments;
        let count = segments.len();
        if count == 0 {
            return RideView { at_ms: now_ms, finished: true, ..RideView::default() };
        }
        let song = plan.songs.get(queue_index);
        let finished = !plan.songs.is_empty() && song.is_none();
        let index = match song {
            Some(s) => s.segment_index,
            None if plan.songs.is_empty() => queue_index,
            None => count - 1,
        }
        .min(count - 1);
        let seg = &segments[index];
        let skip_to_next = !finished && song.and_then(|s| s.play_ms).is_some_and(|end| position_ms >= end);
        // the segment's own clock: where the song's start sits in the segment, plus the song's position
        let offset_ms = song.map_or(0, |s| s.at_segment_ms);
        let segment_ms =
            if finished { u64::from(seg.seconds) * MS_PER_S } else { offset_ms.saturating_add(position_ms) };
        let position_s = if finished { seg.seconds } else { (segment_ms / MS_PER_S).min(u64::from(u32::MAX)) as u32 };
        let pulse_ms = if finished { segment_ms } else { position_ms };
        let remaining_s = seg.seconds.saturating_sub(position_s);

        let interval = seg.interval.as_ref().map(|iv| interval_view(iv, position_s, seg.seconds));
        let range = match interval {
            Some(IntervalView { phase: IntervalPhase::Rest, .. }) => {
                seg.interval.map(|iv| iv.rest_cadence).unwrap_or_else(|| seg.cadence())
            }
            _ => seg.cadence(),
        };

        let total_s = plan.workout.total_seconds();
        let before: u32 = segments[..index].iter().map(|s| s.seconds).sum();
        let elapsed_s = (before + position_s.min(seg.seconds)).min(total_s);
        let next = segments.get(index + 1).map(|n| NextView {
            kind: n.kind,
            position: n.position,
            target_rpm: n.cadence().center(),
            shown: bike.target(n.cadence()),
            level: bike.level(n.effort_at(0)),
            seconds: n.seconds,
        });
        let effort = seg.effort_at(position_s);
        let level = bike.level(effort);

        RideView {
            at_ms: now_ms,
            finished,
            segment_index: index as u32,
            segment_count: count as u32,
            kind: seg.kind,
            position: seg.position,
            effort,
            target_rpm: range.center(),
            range,
            shown: bike.target(range),
            level,
            level_count: if level.is_some() { bike.resistance_levels.min(MAX_RESISTANCE_LEVELS) } else { 0 },
            step: if finished { None } else { step_view(seg, position_s) },
            segment_seconds: seg.seconds,
            segment_start_s: before,
            remaining_s,
            segment_progress: ratio(position_s.min(seg.seconds), seg.seconds),
            elapsed_s,
            total_s,
            progress: ratio(elapsed_s, total_s),
            marker: ProfilePoint { t_s: elapsed_s, elevation: marker_elevation(plan, segment_ms, before) },
            interval,
            notice: !finished && next.is_some() && remaining_s <= u32::from(plan.cues.notice_seconds),
            next,
            pulse: pulse_view(song, range, pulse_ms),
            cues: cues_of(plan, index, segment_ms, finished),
            skip_to_next,
        }
    }
}

/// The elevation under the rider, moving smoothly inside the second.
fn marker_elevation(plan: &RidePlan, position_ms: u64, segment_start_s: u32) -> f32 {
    let t = segment_start_s as f32 + position_ms as f32 / MS_PER_S as f32;
    if plan.profile.is_empty() {
        elevation_at(&profile(&plan.workout), t)
    } else {
        elevation_at(&plan.profile, t)
    }
}

/// The resistance step at `position_s` of `seg`, `None` without steps.
fn step_view(seg: &Segment, position_s: u32) -> Option<StepView> {
    if seg.steps.is_empty() {
        return None;
    }
    let passed = seg.steps.iter().filter(|s| s.at_s <= position_s).count();
    Some(StepView {
        index: (passed + 1).min(usize::from(u8::MAX)) as u8,
        count: (seg.steps.len() + 1).min(usize::from(u8::MAX)) as u8,
        effort: seg.effort_at(position_s),
        next_in_s: seg.steps.get(passed).map(|s| s.at_s - position_s),
    })
}

fn ratio(part: u32, whole: u32) -> f32 {
    if whole == 0 {
        0.0
    } else {
        (part as f32 / whole as f32).clamp(0.0, 1.0)
    }
}

/// The interval phase at `position_s` into the segment: work first, then rest, `rounds` times; any time
/// left after the last round is rest.
fn interval_view(iv: &Interval, position_s: u32, seconds: u32) -> IntervalView {
    let cycle = iv.cycle_s().max(1);
    let round = position_s / cycle;
    if round >= u32::from(iv.rounds) {
        return IntervalView {
            phase: IntervalPhase::Rest,
            round: iv.rounds,
            rounds: iv.rounds,
            phase_remaining_s: seconds.saturating_sub(position_s),
        };
    }
    let offset = position_s % cycle;
    let (phase, phase_remaining_s) = if offset < iv.work_s {
        (IntervalPhase::Work, iv.work_s - offset)
    } else {
        (IntervalPhase::Rest, cycle - offset)
    };
    IntervalView { phase, round: round as u8 + 1, rounds: iv.rounds, phase_remaining_s }
}

fn pulse_view(song: Option<&PlannedSong>, range: RpmRange, position_ms: u64) -> PulseView {
    let center = f64::from(range.center().max(1));
    let (source, first_beat_ms, beat_ms, ratio) = match song {
        Some(s) => match (s.pulse, s.grid, s.bpm) {
            (PulseSource::SongBeat, Some(grid), _) => {
                let ratio = nearest_ratio(grid.bpm() as f32, range);
                (PulseSource::SongBeat, grid.first_beat_ms, grid.beat_ms, ratio)
            }
            (_, _, Some(bpm)) if bpm > 0 => {
                let ratio = nearest_ratio(f32::from(bpm), range);
                (PulseSource::Metronome, 0.0, MS_PER_MINUTE / f64::from(bpm), ratio)
            }
            _ => (PulseSource::Metronome, 0.0, MS_PER_MINUTE / center, BeatRatio::Whole),
        },
        None => (PulseSource::Metronome, 0.0, MS_PER_MINUTE / center, BeatRatio::Whole),
    };
    let stroke_ms = beat_ms / f64::from(ratio.strokes_per_beat());
    PulseView { source, first_beat_ms, beat_ms, stroke_ms, ratio, song_position_ms: position_ms }
}

/// The sound for a segment starting after `prev`.
fn entry_cue(prev: Option<&Segment>, seg: &Segment) -> CueKind {
    if seg.interval.is_some() {
        CueKind::SprintGo
    } else if seg.position.is_standing() && !prev.is_some_and(|p| p.position.is_standing()) {
        CueKind::Standing
    } else if !seg.position.is_standing() && prev.is_some_and(|p| p.position.is_standing()) {
        CueKind::Seated
    } else {
        CueKind::Change
    }
}

fn countdown_kind(k: u32) -> CueKind {
    match k {
        1 => CueKind::Countdown1,
        2 => CueKind::Countdown2,
        _ => CueKind::Countdown3,
    }
}

/// Ids keep a segment's cues apart: `segment * 1000 + slot`.
const ID_PER_SEGMENT: u32 = 1000;
const SLOT_TRANSITION: u32 = 10;
/// Resistance steps sit between the transitions (10..) and the countdowns (100..).
const SLOT_STEP: u32 = 50;
const SLOT_COUNTDOWN: u32 = 100;
const COUNTDOWN_SLOTS_PER_TRANSITION: u32 = 12;

/// Every cue of segment `index` (time from its start) in order: the sound that opens it, interval
/// changes, each with its countdown, and the sound that opens the next segment (or the finish).
fn timeline(plan: &RidePlan, index: usize) -> Vec<(u32, CueKind, i64)> {
    let segments = &plan.workout.segments;
    let seg = &segments[index];
    let countdown = u32::from(plan.cues.countdown_seconds);
    let base = index as u32 * ID_PER_SEGMENT;
    let mut out = Vec::new();
    let prev = index.checked_sub(1).map(|i| &segments[i]);
    out.push((base, entry_cue(prev, seg), 0));

    // the moments inside the segment where something changes, with what is heard there
    let mut changes: Vec<(u32, CueKind)> = Vec::new();
    if let Some(iv) = &seg.interval {
        for round in 0..u32::from(iv.rounds) {
            let start = round * iv.cycle_s();
            if round > 0 {
                changes.push((start, CueKind::SprintGo));
            }
            changes.push((start + iv.work_s, CueKind::Recover));
        }
    }
    let end_kind = match segments.get(index + 1) {
        Some(next) => entry_cue(Some(seg), next),
        None => CueKind::Finish,
    };
    let end_id = (index as u32 + 1) * ID_PER_SEGMENT;
    // an interval that ends on a rest start has the segment's end right after: keep both only if apart
    changes.retain(|(at, _)| *at < seg.seconds);
    changes.sort_by_key(|(at, _)| *at);

    let mut previous_at = 0u32;
    for (n, (at, kind)) in changes.iter().enumerate() {
        out.push((base + SLOT_TRANSITION + n as u32, *kind, i64::from(*at) * MS_PER_S as i64));
        push_countdown(&mut out, *at, previous_at, countdown, base, n as u32);
        previous_at = *at;
    }
    // a resistance step is one soft sound, with no countdown (that would be a shower of beeps on a climb)
    for (k, step) in seg.steps.iter().enumerate().filter(|(_, s)| s.at_s < seg.seconds) {
        out.push((base + SLOT_STEP + k as u32 + 1, CueKind::Resistance, i64::from(step.at_s) * MS_PER_S as i64));
    }
    let end_slot = changes.len() as u32;
    // the end cue's id is the next segment's opener, so both views agree
    out.push((end_id, end_kind, i64::from(seg.seconds) * MS_PER_S as i64));
    push_countdown(&mut out, seg.seconds, previous_at, countdown, base, end_slot);
    out
}

/// "3, 2, 1" ending at `at_s`, only when the phase before it is long enough to hold it.
fn push_countdown(
    out: &mut Vec<(u32, CueKind, i64)>,
    at_s: u32,
    previous_at_s: u32,
    countdown: u32,
    base: u32,
    slot: u32,
) {
    if countdown == 0 || at_s < previous_at_s + countdown + MIN_PHASE_MARGIN_S {
        return;
    }
    for k in 1..=countdown {
        let id = base + SLOT_COUNTDOWN + slot * COUNTDOWN_SLOTS_PER_TRANSITION + k;
        out.push((id, countdown_kind(k), i64::from(at_s - k) * MS_PER_S as i64));
    }
}

fn cues_of(plan: &RidePlan, index: usize, position_ms: u64, finished: bool) -> Vec<Cue> {
    if finished {
        return Vec::new();
    }
    let position = position_ms as i64;
    let mut cues: Vec<Cue> = timeline(plan, index)
        .into_iter()
        .map(|(id, kind, at_ms)| Cue { id, kind, in_ms: at_ms - position })
        .filter(|c| (-CUE_LATE_MS..=CUE_HORIZON_MS).contains(&c.in_ms))
        .collect();
    cues.sort_by_key(|c| c.in_ms);
    cues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exercise::bike::{ConsoleUnit, TargetUnit};
    use crate::exercise::builder::{test_plan, CueSettings};
    use crate::exercise::model::{Effort, EffortStep, Target, Workout};

    fn seg(
        kind: SegmentKind,
        seconds: u32,
        rpm: (u16, u16),
        position: Position,
        interval: Option<Interval>,
    ) -> Segment {
        Segment {
            kind,
            seconds,
            target: Target::Cadence { min_rpm: rpm.0, max_rpm: rpm.1 },
            effort: Effort(5),
            position,
            interval,
            ..Segment::default()
        }
    }

    fn plan() -> RidePlan {
        let sprint = Interval { work_s: 20, rest_s: 40, rounds: 3, rest_cadence: RpmRange::new(80, 90) };
        let workout = Workout {
            segments: vec![
                seg(SegmentKind::WarmUp, 120, (80, 90), Position::Seated, None),
                seg(SegmentKind::StandingClimb, 180, (55, 65), Position::Standing, None),
                seg(SegmentKind::Sprint, 200, (100, 120), Position::Seated, Some(sprint)),
            ],
            ..Workout::default()
        };
        test_plan(workout, CueSettings { countdown_seconds: 3, notice_seconds: 10 })
    }

    #[test]
    fn the_segment_follows_the_queue_and_the_timer_follows_the_position() {
        let p = plan();
        let v = RideClock::view(&p, 0, 30_000, 1_000);
        assert_eq!((v.segment_index, v.kind, v.remaining_s, v.target_rpm), (0, SegmentKind::WarmUp, 90, 85));
        assert_eq!(v.total_s, 500);
        assert_eq!(v.elapsed_s, 30);
        assert!(!v.notice && v.next.is_some_and(|n| n.kind == SegmentKind::StandingClimb));
        let v = RideClock::view(&p, 1, 175_000, 0);
        assert_eq!((v.segment_index, v.remaining_s, v.elapsed_s), (1, 5, 120 + 175));
        assert!(v.notice, "5 s before the change");
        assert_eq!(v.position, Position::Standing);
        assert!((v.progress - 295.0 / 500.0).abs() < 1e-6);
    }

    #[test]
    fn pausing_changes_nothing_and_the_end_of_the_queue_finishes() {
        let p = plan();
        let a = RideClock::view(&p, 1, 42_000, 5);
        let b = RideClock::view(&p, 1, 42_000, 5);
        assert_eq!(a, b);
        let end = RideClock::view(&p, 3, 0, 0);
        assert!(end.finished && end.cues.is_empty());
        assert_eq!((end.progress, end.remaining_s), (1.0, 0));
        assert!(RideClock::view(&test_plan(Workout::default(), CueSettings::default()), 0, 0, 0).finished);
    }

    #[test]
    fn interval_phases_run_work_then_rest_inside_the_song() {
        let p = plan();
        let at = |s: u64| RideClock::view(&p, 2, s * 1000, 0);
        let v = at(5);
        let iv = v.interval.unwrap();
        assert_eq!((iv.phase, iv.round, iv.phase_remaining_s, v.target_rpm), (IntervalPhase::Work, 1, 15, 110));
        let v = at(25);
        let iv = v.interval.unwrap();
        assert_eq!((iv.phase, iv.phase_remaining_s, v.target_rpm), (IntervalPhase::Rest, 35, 85));
        let iv = at(60 + 5).interval.unwrap();
        assert_eq!((iv.phase, iv.round), (IntervalPhase::Work, 2));
        let iv = at(190).interval.unwrap();
        assert_eq!((iv.phase, iv.round, iv.phase_remaining_s), (IntervalPhase::Rest, 3, 10), "the tail is rest");
    }

    #[test]
    fn cues_come_with_a_countdown_and_the_right_sound() {
        let p = plan();
        // 5 s before the warm-up ends: the countdown and then the "stand up" sound
        let v = RideClock::view(&p, 0, 115_000, 0);
        let kinds: Vec<(CueKind, i64)> = v.cues.iter().map(|c| (c.kind, c.in_ms)).collect();
        assert_eq!(
            kinds,
            [
                (CueKind::Countdown3, 2000),
                (CueKind::Countdown2, 3000),
                (CueKind::Countdown1, 4000),
                (CueKind::Standing, 5000)
            ]
        );
        // the sprint segment opens with the sprint horn and signals each recovery
        let v = RideClock::view(&p, 2, 0, 0);
        assert_eq!(v.cues[0].kind, CueKind::SprintGo);
        assert!(v.cues.iter().all(|c| c.kind != CueKind::Recover), "20 s ahead is beyond the horizon");
        let v = RideClock::view(&p, 2, 16_500, 0);
        let kinds: Vec<CueKind> = v.cues.iter().map(|c| c.kind).collect();
        assert_eq!(kinds, [CueKind::Countdown3, CueKind::Countdown2, CueKind::Countdown1, CueKind::Recover]);
        // the last segment ends with the finish sound
        let v = RideClock::view(&p, 2, 199_000, 0);
        assert_eq!(v.cues.last().unwrap().kind, CueKind::Finish);
        // the same cue has the same id in the view before and after the change
        let before = RideClock::view(&p, 0, 119_800, 0);
        let after = RideClock::view(&p, 1, 100, 0);
        let opener = before.cues.iter().find(|c| c.kind == CueKind::Standing).unwrap().id;
        assert_eq!(after.cues.iter().find(|c| c.kind == CueKind::Standing).unwrap().id, opener);
    }

    #[test]
    fn no_countdown_when_it_is_off() {
        let mut p = plan();
        p.cues.countdown_seconds = 0;
        let v = RideClock::view(&p, 0, 115_000, 0);
        assert_eq!(v.cues.iter().map(|c| c.kind).collect::<Vec<_>>(), [CueKind::Standing]);
    }

    #[test]
    fn the_marker_moves_along_the_profile() {
        let p = plan();
        let at = |q, ms| RideClock::view(&p, q, ms, 0).marker;
        assert_eq!(at(0, 0).t_s, 0);
        assert!(at(1, 90_000).elevation > at(1, 0).elevation, "standing climb rises");
        assert_eq!(RideClock::view(&p, 1, 0, 0).segment_start_s, 120);
        assert_eq!(p.profile.len(), 4);
    }

    #[test]
    fn the_pulse_comes_from_the_grid_or_a_metronome() {
        use crate::dj::Grid;
        let mut p = plan();
        p.songs[0].grid = Grid::new(100.0, 352.941); // 170 BPM
        p.songs[0].pulse = PulseSource::SongBeat;
        p.songs[0].bpm = Some(170);
        let v = RideClock::view(&p, 0, 1000, 0);
        assert_eq!(v.pulse.source, PulseSource::SongBeat);
        assert_eq!(v.pulse.ratio, BeatRatio::Half, "170 BPM pedaled at half-time for 80..90");
        assert!((v.pulse.stroke_ms - 705.882).abs() < 0.01);
        assert!(v.pulse.stroke_phase(100.0).abs() < 1e-9);
        assert!(v.pulse.next_stroke_in_ms() > 0.0 && v.pulse.next_stroke_in_ms() <= v.pulse.stroke_ms);
        // no grid: a metronome at the song's tempo, or at the cadence when the tempo is unknown
        p.songs[1].grid = None;
        p.songs[1].bpm = None;
        let v = RideClock::view(&p, 1, 0, 0);
        assert_eq!(v.pulse.source, PulseSource::Metronome);
        assert!((v.pulse.stroke_ms - 60_000.0 / 60.0).abs() < 1e-6, "target 55..65 is 60 RPM");
    }

    /// A 300 s seated climb with resistance steps at 100 s and 200 s, then a 120 s cool-down.
    fn climb_plan() -> RidePlan {
        let climb = Segment {
            steps: vec![EffortStep { at_s: 100, effort: Effort(7) }, EffortStep { at_s: 200, effort: Effort(8) }],
            effort: Effort(6),
            ..seg(SegmentKind::SeatedClimb, 300, (60, 70), Position::Seated, None)
        };
        let workout = Workout {
            segments: vec![climb, seg(SegmentKind::CoolDown, 120, (70, 80), Position::Seated, None)],
            ..Workout::default()
        };
        test_plan(workout, CueSettings { countdown_seconds: 3, notice_seconds: 10 })
    }

    #[test]
    fn the_view_shows_the_step_and_the_resistance_cue_sounds_at_each() {
        let p = climb_plan();
        let at = |s: u64| RideClock::view(&p, 0, s * 1000, 0);
        let v = at(50);
        assert_eq!((v.effort, v.step), (6, Some(StepView { index: 1, count: 3, effort: 6, next_in_s: Some(50) })));
        let v = at(100);
        assert_eq!((v.effort, v.step.unwrap().index, v.step.unwrap().next_in_s), (7, 2, Some(100)));
        let v = at(250);
        assert_eq!((v.effort, v.step), (8, Some(StepView { index: 3, count: 3, effort: 8, next_in_s: None })));
        assert!(RideClock::view(&p, 1, 0, 0).step.is_none(), "a segment without steps has none");
        // the cue is listed a few seconds ahead, with its own id, and no countdown comes with it
        let v = RideClock::view(&p, 0, 95_000, 0);
        let kinds: Vec<(CueKind, i64)> = v.cues.iter().map(|c| (c.kind, c.in_ms)).collect();
        assert_eq!(kinds, [(CueKind::Resistance, 5000)]);
        let v = RideClock::view(&p, 0, 195_000, 0);
        let step_ids: Vec<u32> = v.cues.iter().filter(|c| c.kind == CueKind::Resistance).map(|c| c.id).collect();
        assert_eq!(step_ids, [SLOT_STEP + 2]);
        // the end-of-climb countdown is unchanged
        let v = RideClock::view(&p, 0, 295_000, 0);
        let kinds: Vec<CueKind> = v.cues.iter().map(|c| c.kind).collect();
        assert_eq!(kinds, [CueKind::Countdown3, CueKind::Countdown2, CueKind::Countdown1, CueKind::Change]);
    }

    #[test]
    fn the_view_carries_the_shown_target() {
        let p = plan();
        let v = RideClock::view(&p, 0, 0, 0);
        assert_eq!((v.shown.unit, v.shown.rpm, v.level, v.level_count), (TargetUnit::Rpm, 85, Some(4), 8));
        assert!(v.next.is_some_and(|n| n.shown.rpm == 60 && n.level == Some(4)));
        let bike = BikeProfile { console: ConsoleUnit::Kph, kph_per_rpm: 0.2, resistance_levels: 0 };
        let v = RideClock::view_on(&p, &bike, 0, 0, 0);
        assert_eq!((v.shown.unit, v.shown.value, v.target_rpm), (TargetUnit::Kph, 17.0, 85));
        assert_eq!((v.level, v.level_count), (None, 0));
        assert_eq!(v.next.unwrap().shown.value, 12.0);
        // a sprint's rest phase shows the rest cadence, in the same unit
        let v = RideClock::view_on(&p, &bike, 2, 25_000, 0);
        assert_eq!((v.shown.unit, v.shown.rpm), (TargetUnit::Kph, 85));
    }

    #[test]
    fn a_live_swap_keeps_the_segment_clock() {
        let mut p = plan();
        p.songs[1].at_segment_ms = 60_000;
        p.songs[1].play_ms = Some(120_000);
        // 10 s into the new song = 70 s into the 180 s segment
        let v = RideClock::view(&p, 1, 10_000, 0);
        assert_eq!((v.remaining_s, v.elapsed_s, v.segment_progress), (110, 120 + 70, 70.0 / 180.0));
        assert!(!v.skip_to_next);
        // the pulse reads the song's own position
        assert_eq!(v.pulse.song_position_ms, 10_000);
        // 60 s into the new song is the segment's end: the cut is at the song's play_ms
        let v = RideClock::view(&p, 1, 120_000, 0);
        assert!(v.skip_to_next && v.remaining_s == 0);
        // the cues follow the segment: the change is 5 s away at 115 s into the song
        let v = RideClock::view(&p, 1, 115_000, 0);
        assert_eq!(v.cues.iter().find(|c| c.kind == CueKind::Countdown3).map(|c| c.in_ms), Some(2000));
    }

    #[test]
    fn old_plans_read_with_at_segment_ms_zero() {
        let old: PlannedSong = serde_json::from_str(r#"{"segment_index":2,"play_ms":90000}"#).unwrap();
        assert_eq!((old.segment_index, old.at_segment_ms), (2, 0));
        let v = RideClock::view(&plan(), 0, 30_000, 0);
        assert_eq!(v.remaining_s, 90, "no offset: the song's clock is the segment's");
    }
}
