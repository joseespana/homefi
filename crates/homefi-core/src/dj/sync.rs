//! Matching two songs: the tempo that makes one follow the other, and the shift that puts
//! their beats on top of each other.

use serde::{Deserialize, Serialize};

/// A constant-tempo beat grid: beat `n` is at `first_beat_ms + n * beat_ms` (n may be negative:
/// the grid extends before the first beat and after the last).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Grid {
    pub first_beat_ms: f64,
    pub beat_ms: f64,
}

impl Grid {
    /// `None` when the numbers can't describe a grid.
    pub fn new(first_beat_ms: f64, beat_ms: f64) -> Option<Self> {
        (first_beat_ms.is_finite() && beat_ms.is_finite() && beat_ms > 0.0).then_some(Self { first_beat_ms, beat_ms })
    }

    pub fn bpm(&self) -> f64 {
        60_000.0 / self.beat_ms
    }

    /// Beats from the first one to `ms` (fractional).
    pub fn beats_at(&self, ms: f64) -> f64 {
        (ms - self.first_beat_ms) / self.beat_ms
    }

    /// Where `ms` is inside its beat, 0..1 (0 = on the beat).
    pub fn phase(&self, ms: f64) -> f64 {
        self.beats_at(ms).rem_euclid(1.0)
    }

    /// The beat closest to `ms`.
    pub fn nearest_beat_ms(&self, ms: f64) -> f64 {
        self.first_beat_ms + self.beats_at(ms).round() * self.beat_ms
    }

    /// The first beat strictly after `ms`.
    pub fn next_beat_ms(&self, ms: f64) -> f64 {
        self.first_beat_ms + (self.beats_at(ms).floor() + 1.0) * self.beat_ms
    }
}

/// Playback speed (1.0 = normal) that gives a song of `own_bpm` the tempo `target_bpm`, taking
/// the half or double tempo when that is closer (a 70 BPM song follows a 140 BPM one at its own
/// speed). `None` when no choice fits in `+/- range_pct` percent.
pub fn tempo_for_sync(own_bpm: f64, target_bpm: f64, range_pct: f64) -> Option<f64> {
    if !(own_bpm.is_finite() && target_bpm.is_finite()) || own_bpm <= 0.0 || target_bpm <= 0.0 {
        return None;
    }
    let range = range_pct.abs() / 100.0;
    [1.0, 0.5, 2.0]
        .into_iter()
        .map(|fold| target_bpm * fold / own_bpm)
        .filter(|ratio| (ratio - 1.0).abs() <= range + 1e-9)
        .min_by(|a, b| (a - 1.0).abs().total_cmp(&(b - 1.0).abs()))
}

/// How far (ms of the song) the `follower` must move so its beat lines up with the `leader`'s,
/// the shortest way (within half a beat of the follower, positive = forward). Both are
/// positions in their own song; the tempos are assumed already matched.
pub fn phase_offset_ms(leader: &Grid, leader_pos_ms: f64, follower: &Grid, follower_pos_ms: f64) -> f64 {
    let diff = leader.phase(leader_pos_ms) - follower.phase(follower_pos_ms);
    // wrap into -0.5..0.5 of a beat
    let wrapped = (diff + 0.5).rem_euclid(1.0) - 0.5;
    wrapped * follower.beat_ms
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(first: f64, bpm: f64) -> Grid {
        Grid::new(first, 60_000.0 / bpm).unwrap()
    }

    #[test]
    fn grid_phase_and_beats() {
        let g = grid(100.0, 120.0); // beat every 500 ms
        assert_eq!(g.bpm(), 120.0);
        assert!(g.phase(100.0).abs() < 1e-9 && g.phase(600.0).abs() < 1e-9);
        assert!((g.phase(350.0) - 0.5).abs() < 1e-9);
        assert!((g.phase(0.0) - 0.8).abs() < 1e-9, "before the first beat");
        assert_eq!(g.nearest_beat_ms(790.0), 600.0);
        assert_eq!(g.next_beat_ms(600.0), 1100.0);
        assert_eq!(g.next_beat_ms(599.0), 600.0);
        assert!(Grid::new(0.0, 0.0).is_none() && Grid::new(f64::NAN, 500.0).is_none());
    }

    #[test]
    fn tempo_sync_respects_the_range_and_folds_octaves() {
        let r = tempo_for_sync(124.0, 128.0, 8.0).unwrap();
        assert!((r - 128.0 / 124.0).abs() < 1e-12);
        assert!(tempo_for_sync(100.0, 128.0, 8.0).is_none(), "28 % is out of range");
        let half = tempo_for_sync(70.0, 140.0, 8.0).unwrap();
        assert!((half - 1.0).abs() < 1e-12, "plays at its own speed against the double tempo");
        let double = tempo_for_sync(140.0, 72.0, 8.0).unwrap();
        assert!((double - 144.0 / 140.0).abs() < 1e-12);
        assert!(tempo_for_sync(0.0, 120.0, 8.0).is_none() && tempo_for_sync(f64::NAN, 120.0, 8.0).is_none());
        // a wider range accepts what a narrow one refuses
        assert!(tempo_for_sync(100.0, 128.0, 50.0).is_some());
    }

    #[test]
    fn phase_offset_takes_the_short_way() {
        let a = grid(0.0, 120.0); // 500 ms beats
        let b = grid(50.0, 120.0);
        // leader exactly on a beat, follower 100 ms after its beat: move back 100 ms
        assert!((phase_offset_ms(&a, 1000.0, &b, 650.0) + 100.0).abs() < 1e-9);
        // follower 100 ms before its beat: move forward
        assert!((phase_offset_ms(&a, 1000.0, &b, 450.0) - 100.0).abs() < 1e-9);
        // aligned already
        assert!(phase_offset_ms(&a, 1500.0, &b, 550.0).abs() < 1e-9);
        // never more than half a beat
        for p in 0..500 {
            assert!(phase_offset_ms(&a, 0.0, &b, p as f64).abs() <= 250.0 + 1e-9);
        }
    }
}
