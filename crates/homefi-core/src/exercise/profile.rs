//! The course profile, like the mountain chart of a spin class: heavy segments climb, easy ones are flat,
//! recovery and cool-down descend. Pure; the elevation is a drawing aid, not a measurement.

use serde::{Deserialize, Serialize};

use super::model::{Segment, SegmentKind, Workout};

/// Effort at and below which a segment does not climb (a warm-up's 3).
const FLAT_EFFORT: f32 = 3.0;
/// Effort span above [`FLAT_EFFORT`] up to 10.
const EFFORT_SPAN: f32 = 7.0;
const DESCENT_RECOVERY: f32 = -0.5;
const DESCENT_COOL_DOWN: f32 = -0.6;

/// A point of the profile: seconds from the start and elevation 0..=1 (normalized for drawing).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProfilePoint {
    pub t_s: u32,
    pub elevation: f32,
}

/// How steep a segment draws, per second of the ride (before normalizing).
pub fn grade(seg: &Segment) -> f32 {
    let climb = |factor: f32| factor * ((f32::from(seg.effort.get()) - FLAT_EFFORT) / EFFORT_SPAN).max(0.0);
    match seg.kind {
        SegmentKind::Recovery => DESCENT_RECOVERY,
        SegmentKind::CoolDown => DESCENT_COOL_DOWN,
        SegmentKind::StandingClimb => climb(1.2),
        SegmentKind::SeatedClimb => climb(1.0),
        SegmentKind::Sprint => climb(0.9),
        SegmentKind::Jumps => climb(0.6),
        SegmentKind::SeatedFlat => climb(0.3),
        SegmentKind::WarmUp => climb(0.15),
    }
}

/// A resistance step on the course ("one more turn" at `t_s` seconds into the ride): the renderers mark it
/// on the ridge, whose elevation they read with [`elevation_at`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct HotPoint {
    pub t_s: u32,
    /// The effort from here on.
    pub effort: u8,
    /// 1-based level of the climb that starts here; the climb's own start is level 1, so the first hot point is 2.
    pub level: u8,
    /// Levels of the climb.
    pub count: u8,
}

/// Every resistance step of the workout, in ride order. Pure: it reads the segments' steps.
pub fn hot_points(workout: &Workout) -> Vec<HotPoint> {
    let mut start_s = 0u32;
    let mut out = Vec::new();
    for seg in &workout.segments {
        let count = (seg.steps.len() + 1).min(usize::from(u8::MAX)) as u8;
        out.extend(seg.steps.iter().enumerate().map(|(k, step)| HotPoint {
            t_s: start_s + step.at_s,
            effort: step.effort.get(),
            level: (k + 2).min(usize::from(u8::MAX)) as u8,
            count,
        }));
        start_s += seg.seconds;
    }
    out
}

/// One point at the start and one at the end of every segment, elevations normalized to 0..=1
/// (0.5 everywhere for a ride with no relief). Time never goes backwards.
pub fn profile(workout: &Workout) -> Vec<ProfilePoint> {
    let mut raw = vec![(0u32, 0.0f32)];
    let (mut t, mut e) = (0u32, 0.0f32);
    for seg in &workout.segments {
        t += seg.seconds;
        e += grade(seg) * seg.seconds as f32;
        raw.push((t, e));
    }
    let (low, high) = raw.iter().fold((f32::MAX, f32::MIN), |(lo, hi), (_, e)| (lo.min(*e), hi.max(*e)));
    let span = high - low;
    raw.into_iter()
        .map(|(t_s, e)| ProfilePoint { t_s, elevation: if span > f32::EPSILON { (e - low) / span } else { 0.5 } })
        .collect()
}

/// The elevation at `t_s` seconds into the ride, between the two points around it.
pub fn elevation_at(points: &[ProfilePoint], t_s: f32) -> f32 {
    let (Some(first), Some(last)) = (points.first(), points.last()) else { return 0.0 };
    if t_s <= first.t_s as f32 {
        return first.elevation;
    }
    if t_s >= last.t_s as f32 {
        return last.elevation;
    }
    points
        .windows(2)
        .find(|w| t_s <= w[1].t_s as f32)
        .map(|w| {
            let span = (w[1].t_s - w[0].t_s) as f32;
            if span <= 0.0 {
                w[1].elevation
            } else {
                w[0].elevation + (w[1].elevation - w[0].elevation) * (t_s - w[0].t_s as f32) / span
            }
        })
        .unwrap_or(last.elevation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exercise::model::{Effort, PresetId};
    use crate::exercise::presets::preset;
    use crate::exercise::settings::ExerciseSettings;

    fn seg(kind: SegmentKind, seconds: u32, effort: u8) -> Segment {
        Segment { kind, seconds, effort: Effort(effort), ..Segment::default() }
    }

    #[test]
    fn climbs_rise_recovery_falls_and_flat_stays_level() {
        let w = Workout {
            segments: vec![
                seg(SegmentKind::WarmUp, 120, 3),
                seg(SegmentKind::SeatedClimb, 240, 7),
                seg(SegmentKind::Recovery, 120, 3),
                seg(SegmentKind::SeatedFlat, 100, 3),
                seg(SegmentKind::CoolDown, 120, 2),
            ],
            ..Workout::default()
        };
        let p = profile(&w);
        assert_eq!(p.len(), 6);
        assert!(p[2].elevation > p[1].elevation, "the climb rises");
        assert!(p[3].elevation < p[2].elevation, "recovery falls");
        assert_eq!(p[1].elevation, p[0].elevation, "an easy warm-up is flat");
        assert_eq!(p[4].elevation, p[3].elevation, "an easy flat is level");
        assert!(p[5].elevation < p[4].elevation, "the cool-down descends");
    }

    #[test]
    fn harder_climbs_are_steeper_and_standing_beats_seated() {
        assert!(grade(&seg(SegmentKind::SeatedClimb, 60, 8)) > grade(&seg(SegmentKind::SeatedClimb, 60, 6)));
        assert!(grade(&seg(SegmentKind::StandingClimb, 60, 7)) > grade(&seg(SegmentKind::SeatedClimb, 60, 7)));
        assert!(grade(&seg(SegmentKind::Recovery, 60, 9)) < 0.0);
    }

    #[test]
    fn every_preset_profile_is_monotonic_in_time_and_normalized() {
        for style in PresetId::ALL {
            for minutes in [10, 30, 47, 90] {
                let w = preset(style, minutes, &ExerciseSettings::default());
                let p = profile(&w);
                assert_eq!(p.len(), w.segments.len() + 1);
                assert!(p.windows(2).all(|w| w[0].t_s < w[1].t_s));
                assert_eq!(p.last().unwrap().t_s, w.total_seconds());
                assert!(p.iter().all(|x| (0.0..=1.0).contains(&x.elevation)));
                let (lo, hi) =
                    p.iter().fold((1.0f32, 0.0f32), |(lo, hi), x| (lo.min(x.elevation), hi.max(x.elevation)));
                assert!(lo.abs() < 1e-6 && (hi - 1.0).abs() < 1e-6, "{style:?} {minutes}: {lo}..{hi}");
            }
        }
        let flat = profile(&Workout { segments: vec![seg(SegmentKind::WarmUp, 60, 3)], ..Workout::default() });
        assert!(flat.iter().all(|p| p.elevation == 0.5));
        assert!(profile(&Workout::default()).len() == 1);
    }

    #[test]
    fn the_marker_interpolates() {
        let p = [ProfilePoint { t_s: 0, elevation: 0.0 }, ProfilePoint { t_s: 100, elevation: 1.0 }];
        assert_eq!(elevation_at(&p, 25.0), 0.25);
        assert_eq!(elevation_at(&p, -5.0), 0.0);
        assert_eq!(elevation_at(&p, 500.0), 1.0);
        assert_eq!(elevation_at(&[], 3.0), 0.0);
    }

    #[test]
    fn hot_points_sit_at_segment_start_plus_at_s() {
        use crate::exercise::model::EffortStep;
        let step = |at_s, effort| EffortStep { at_s, effort: Effort(effort) };
        let w = Workout {
            segments: vec![
                seg(SegmentKind::WarmUp, 120, 3),
                Segment { steps: vec![step(100, 7), step(200, 8)], ..seg(SegmentKind::SeatedClimb, 300, 6) },
                Segment { steps: vec![step(60, 9)], ..seg(SegmentKind::StandingClimb, 120, 8) },
            ],
            ..Workout::default()
        };
        let points = hot_points(&w);
        let got: Vec<(u32, u8, u8, u8)> = points.iter().map(|p| (p.t_s, p.effort, p.level, p.count)).collect();
        assert_eq!(got, [(220, 7, 2, 3), (320, 8, 3, 3), (480, 9, 2, 2)]);
        assert!(hot_points(&Workout::default()).is_empty());
    }
}
