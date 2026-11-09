//! The ride styles, at any length, and the beginner caps.
//!
//! A style is a warm-up, a main cycle that repeats as often as the length asks, and a cool-down. The
//! warm-up and cool-down keep sane shares of the ride (15 % and 12 %, within limits); each cycle splits
//! its time by block weights, drops its optional blocks when it is too short for them, and a sprint block
//! scales by rounds, not by stretching the work.

use super::model::{
    round_minutes, Effort, Interval, Position, PresetId, RpmRange, Segment, SegmentKind, Sport, Target, Workout,
    WorkoutOrigin, MAX_RIDE_MINUTES, MIN_RIDE_MINUTES, WORKOUT_FORMAT_VERSION,
};
use super::settings::ExerciseSettings;
use crate::playlist::new_id;

const SECONDS_PER_MINUTE: u32 = 60;
/// Times are kept to multiples of this (s), so the timer reads in tidy numbers.
const SNAP_S: u32 = 5;
const WARM_UP_SHARE: f32 = 0.15;
const COOL_DOWN_SHARE: f32 = 0.12;
const MIN_WARM_UP_S: u32 = 120;
const MAX_WARM_UP_S: u32 = 480;
const MIN_COOL_DOWN_S: u32 = 90;
const MAX_COOL_DOWN_S: u32 = 420;
const MAX_CYCLES: u32 = 6;

/// Beginner caps (see `docs/EXERCISE_PLAN.md`).
pub const BEGINNER_MIN_RPM: u16 = 60;
pub const BEGINNER_MAX_RPM: u16 = 110;
pub const BEGINNER_MAX_EFFORT: u8 = 8;
pub const BEGINNER_MAX_WORK_S: u32 = 30;
/// Rest is at least this many times the work.
pub const BEGINNER_REST_FACTOR: u32 = 2;

/// A part of a style.
struct Block {
    kind: SegmentKind,
    /// Share of the cycle's time.
    weight: u32,
    min_s: u32,
    optional: bool,
    rpm: (u16, u16),
    effort: u8,
    position: Position,
    sprint: Option<Sprint>,
}

struct Sprint {
    work_s: u32,
    rest_s: u32,
    rest_rpm: (u16, u16),
    max_rounds: u8,
}

fn block(kind: SegmentKind, weight: u32, min_s: u32, rpm: (u16, u16), effort: u8, position: Position) -> Block {
    Block { kind, weight, min_s, optional: false, rpm, effort, position, sprint: None }
}

fn optional(b: Block) -> Block {
    Block { optional: true, ..b }
}

fn sprint(weight: u32, work_s: u32, rest_s: u32, max_rounds: u8) -> Block {
    Block {
        sprint: Some(Sprint { work_s, rest_s, rest_rpm: (80, 90), max_rounds }),
        // the block's own minimum is one round
        ..block(SegmentKind::Sprint, weight, work_s + rest_s, (100, 120), 9, Position::Seated)
    }
}

struct Style {
    warm_up: Block,
    /// Minutes one pass of `main` is meant to take; the ride repeats it `length / cycle` times.
    cycle_minutes: u32,
    main: Vec<Block>,
    cool_down: Block,
}

fn style(id: PresetId) -> Style {
    use SegmentKind::*;
    let seated = Position::Seated;
    let warm_up = block(WarmUp, 1, 0, (80, 95), 3, seated);
    let cool_down = block(CoolDown, 1, 0, (70, 85), 2, seated);
    let flat = |weight| block(SeatedFlat, weight, 90, (85, 100), 5, seated);
    let climb = |weight| block(SeatedClimb, weight, 90, (60, 80), 7, seated);
    let recovery = |weight| optional(block(Recovery, weight, 45, (80, 90), 3, seated));
    match id {
        PresetId::FirstRide => {
            Style { warm_up, cycle_minutes: 10, main: vec![flat(4), climb(4), recovery(2)], cool_down }
        }
        PresetId::Easy => Style {
            warm_up,
            cycle_minutes: 12,
            main: vec![
                flat(3),
                optional(block(Jumps, 2, 60, (80, 90), 6, Position::jumps())),
                climb(3),
                optional(sprint(3, 20, 60, 4)),
                recovery(1),
            ],
            cool_down,
        },
        PresetId::Classic => Style {
            warm_up,
            cycle_minutes: 15,
            main: vec![
                flat(3),
                optional(block(StandingClimb, 3, 75, (55, 70), 7, Position::Standing)),
                climb(3),
                optional(sprint(3, 30, 60, 6)),
                recovery(1),
            ],
            cool_down,
        },
        PresetId::Intervals => {
            Style { warm_up, cycle_minutes: 10, main: vec![flat(2), sprint(5, 20, 40, 6), recovery(2)], cool_down }
        }
    }
}

fn snap(seconds: u32) -> u32 {
    (seconds + SNAP_S / 2) / SNAP_S * SNAP_S
}

fn segment_of(b: &Block, seconds: u32) -> Segment {
    let (min_rpm, max_rpm) = b.rpm;
    let mut seg = Segment {
        kind: b.kind,
        seconds,
        target: Target::Cadence { min_rpm, max_rpm },
        effort: Effort(b.effort),
        position: b.position,
        interval: None,
        note: String::new(),
    };
    if let Some(s) = &b.sprint {
        let cycle = s.work_s + s.rest_s;
        let rounds = (seconds / cycle).clamp(1, u32::from(s.max_rounds)) as u8;
        seg.interval = Some(Interval {
            work_s: s.work_s,
            rest_s: s.rest_s,
            rounds,
            rest_cadence: RpmRange::new(s.rest_rpm.0, s.rest_rpm.1),
        });
        seg.seconds = cycle * u32::from(rounds);
    }
    seg
}

/// One pass of the main blocks within `cycle_s` seconds.
fn cycle(blocks: &[Block], cycle_s: u32) -> Vec<Segment> {
    // drop optional blocks from the end until the minimums fit
    let mut kept: Vec<&Block> = blocks.iter().collect();
    while kept.iter().map(|b| b.min_s).sum::<u32>() > cycle_s {
        match kept.iter().rposition(|b| b.optional) {
            Some(i) => {
                kept.remove(i);
            }
            None => break,
        }
    }
    let spare = cycle_s.saturating_sub(kept.iter().map(|b| b.min_s).sum());
    let weights: u32 = kept.iter().map(|b| b.weight).sum::<u32>().max(1);
    let mut segments: Vec<Segment> = kept
        .iter()
        .map(|b| segment_of(b, snap(b.min_s + (u64::from(spare) * u64::from(b.weight) / u64::from(weights)) as u32)))
        .collect();
    // a sprint block that could not use all of its time hands the rest to the first plain block
    let used: u32 = segments.iter().map(|s| s.seconds).sum();
    if let Some(slack_at) = segments.iter().position(|s| s.interval.is_none()) {
        let seg = &mut segments[slack_at];
        seg.seconds = (seg.seconds + cycle_s).saturating_sub(used).max(SNAP_S);
    }
    segments
}

/// `style` at `minutes` (rounded to 5 and kept in 10..=90): warm-up, the main cycle repeated, cool-down,
/// with the beginner caps applied when `settings.beginner_mode`. Total length is the request exactly,
/// give or take a few seconds of snapping.
pub fn preset(style_id: PresetId, minutes: u16, settings: &ExerciseSettings) -> Workout {
    let minutes = round_minutes(minutes.clamp(MIN_RIDE_MINUTES, MAX_RIDE_MINUTES));
    let total_s = u32::from(minutes) * SECONDS_PER_MINUTE;
    let style = style(style_id);
    let warm_s = snap(((total_s as f32 * WARM_UP_SHARE) as u32).clamp(MIN_WARM_UP_S, MAX_WARM_UP_S));
    let cool_s = snap(((total_s as f32 * COOL_DOWN_SHARE) as u32).clamp(MIN_COOL_DOWN_S, MAX_COOL_DOWN_S));
    let main_s = total_s.saturating_sub(warm_s + cool_s);
    let cycles =
        ((main_s as f32 / (style.cycle_minutes * SECONDS_PER_MINUTE) as f32).round() as u32).clamp(1, MAX_CYCLES);

    let mut segments = vec![segment_of(&style.warm_up, warm_s)];
    for _ in 0..cycles {
        segments.extend(cycle(&style.main, main_s / cycles));
    }
    segments.push(segment_of(&style.cool_down, cool_s));

    // snapping and sprint rounds leave a few seconds either way: the longest plain segment takes them
    let sum: u32 = segments.iter().map(|s| s.seconds).sum();
    if let Some(seg) = segments.iter_mut().filter(|s| s.interval.is_none()).max_by_key(|s| s.seconds) {
        seg.seconds = (seg.seconds + total_s).saturating_sub(sum);
    }
    let workout = Workout {
        version: WORKOUT_FORMAT_VERSION,
        id: new_id(),
        name: String::new(),
        sport: Sport::IndoorCycling,
        segments,
        origin: WorkoutOrigin::Preset { preset: style_id },
    }
    .sanitized();
    beginner_caps(&workout, settings)
}

/// The workout with the beginner caps applied (a copy; nothing when `beginner_mode` is off): cadence
/// 60..110, standing (and jumps) never under 60 RPM, sprint work at most 30 s with at least twice the
/// rest, effort at most 8. A sprint block keeps its length, with fewer rounds if the longer rest asks for it.
pub fn beginner_caps(workout: &Workout, settings: &ExerciseSettings) -> Workout {
    if !settings.beginner_mode {
        return workout.clone();
    }
    let mut out = workout.clone();
    for seg in &mut out.segments {
        let range = seg.cadence();
        let floor = BEGINNER_MIN_RPM;
        let (min_rpm, max_rpm) =
            (range.min_rpm.clamp(floor, BEGINNER_MAX_RPM), range.max_rpm.clamp(floor, BEGINNER_MAX_RPM));
        seg.target = Target::Cadence { min_rpm, max_rpm };
        seg.effort = Effort(seg.effort.get().min(BEGINNER_MAX_EFFORT));
        if let Some(iv) = &mut seg.interval {
            iv.work_s = iv.work_s.min(BEGINNER_MAX_WORK_S);
            iv.rest_s = iv.rest_s.max(iv.work_s * BEGINNER_REST_FACTOR);
            let rest = iv.rest_cadence;
            iv.rest_cadence =
                RpmRange::new(rest.min_rpm.clamp(floor, BEGINNER_MAX_RPM), rest.max_rpm.clamp(floor, BEGINNER_MAX_RPM));
        }
    }
    // the longer rest may not fit as many rounds: lower them, give the left-over time to the interval's rest
    let mut result = out.sanitized();
    for seg in &mut result.segments {
        if let Some(iv) = &mut seg.interval {
            let rounds = (seg.seconds / iv.cycle_s()).clamp(1, u32::from(iv.rounds)) as u8;
            iv.rounds = rounds;
        }
    }
    result
}

#[cfg(test)]
pub(crate) fn assert_valid(w: &Workout, settings: &ExerciseSettings) {
    for seg in &w.segments {
        let r = seg.cadence();
        assert!(r.min_rpm <= r.max_rpm);
        if let Some(iv) = seg.interval {
            assert!(iv.total_s() <= seg.seconds, "{:?} rounds must fit", seg.kind);
        }
        if settings.beginner_mode {
            assert!(r.min_rpm >= BEGINNER_MIN_RPM && r.max_rpm <= BEGINNER_MAX_RPM, "{:?} cadence {r:?}", seg.kind);
            assert!(seg.effort.get() <= BEGINNER_MAX_EFFORT);
            if seg.position.is_standing() {
                assert!(r.min_rpm >= BEGINNER_MIN_RPM);
            }
            if let Some(iv) = seg.interval {
                assert!(iv.work_s <= BEGINNER_MAX_WORK_S);
                assert!(iv.rest_s >= BEGINNER_REST_FACTOR * iv.work_s);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LENGTHS: [u16; 6] = [10, 20, 25, 47, 60, 90];

    #[test]
    fn every_style_at_every_length_sums_to_the_request_and_respects_the_caps() {
        let settings = ExerciseSettings::default();
        for style in PresetId::ALL {
            for minutes in LENGTHS {
                let w = preset(style, minutes, &settings);
                let asked = u32::from(round_minutes(minutes)) * 60;
                let got = w.total_seconds();
                // beginner caps may stretch a sprint block's rest, never by much
                assert!(got.abs_diff(asked) * 5 <= asked, "{style:?} {minutes} min: {got} s vs {asked} s");
                assert_valid(&w, &settings);
                assert_eq!(w.segments.first().unwrap().kind, SegmentKind::WarmUp);
                assert_eq!(w.segments.last().unwrap().kind, SegmentKind::CoolDown);
            }
        }
    }

    #[test]
    fn styles_have_the_blocks_they_promise() {
        let s = ExerciseSettings::default();
        let kinds = |style, m| -> Vec<SegmentKind> { preset(style, m, &s).segments.iter().map(|x| x.kind).collect() };
        let first = kinds(PresetId::FirstRide, 20);
        assert_eq!(
            first,
            [
                SegmentKind::WarmUp,
                SegmentKind::SeatedFlat,
                SegmentKind::SeatedClimb,
                SegmentKind::Recovery,
                SegmentKind::CoolDown
            ]
        );
        let easy = kinds(PresetId::Easy, 30);
        assert!(easy.contains(&SegmentKind::Jumps) && easy.contains(&SegmentKind::Sprint));
        let classic = kinds(PresetId::Classic, 45);
        assert!(classic.contains(&SegmentKind::StandingClimb));
        assert_eq!(classic.iter().filter(|k| **k == SegmentKind::Sprint).count(), 2, "two interval blocks");
        assert!(kinds(PresetId::Intervals, 30).iter().filter(|k| **k == SegmentKind::Sprint).count() >= 2);
    }

    #[test]
    fn longer_rides_repeat_and_shorter_ones_drop_blocks() {
        let s = ExerciseSettings::default();
        let n = |m| preset(PresetId::Classic, m, &s).segments.len();
        assert!(n(10) < n(45) && n(45) < n(90), "{} {} {}", n(10), n(45), n(90));
        assert_eq!(preset(PresetId::Easy, 10, &s).segments.iter().filter(|x| x.kind == SegmentKind::WarmUp).count(), 1);
    }

    #[test]
    fn the_caps_change_a_wild_workout_and_can_be_switched_off() {
        let wild = Workout {
            segments: vec![
                Segment {
                    kind: SegmentKind::StandingClimb,
                    position: Position::Standing,
                    target: Target::Cadence { min_rpm: 40, max_rpm: 130 },
                    effort: Effort(10),
                    ..Segment::default()
                },
                Segment {
                    kind: SegmentKind::Sprint,
                    seconds: 360,
                    target: Target::Cadence { min_rpm: 110, max_rpm: 125 },
                    interval: Some(Interval { work_s: 45, rest_s: 45, rounds: 4, rest_cadence: RpmRange::new(40, 70) }),
                    ..Segment::default()
                },
            ],
            ..Workout::default()
        }
        .sanitized();
        let on = ExerciseSettings::default();
        let capped = beginner_caps(&wild, &on);
        assert_valid(&capped, &on);
        let iv = capped.segments[1].interval.unwrap();
        assert_eq!((iv.work_s, iv.rest_s), (30, 60));
        assert_eq!(capped.segments[0].cadence(), RpmRange::new(60, 110));
        let off = ExerciseSettings { beginner_mode: false, ..on };
        assert_eq!(beginner_caps(&wild, &off), wild);
        let free = preset(PresetId::Intervals, 30, &off);
        assert!(free.segments.iter().any(|s| s.effort.get() == 9), "sprints are all out without the caps");
    }
}
