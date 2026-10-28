//! Playing along with the hub (party mode): a shared clock and how far a
//! guest's player is from where the hub is. Pure logic; the guest client feeds
//! [`ClockSync`] and the app's player applies [`SyncAction`]s.

use std::collections::VecDeque;

/// Clock samples kept; the one with the shortest round trip wins.
const SAMPLES: usize = 8;

/// The offset between this device's clock and the hub's, from `clock`
/// round trips (like NTP: the fastest round trip is the most trustworthy).
#[derive(Debug, Clone, Default)]
pub struct ClockSync {
    /// `(round trip, hub - local)` in milliseconds, newest last.
    samples: VecDeque<(u64, i64)>,
}

impl ClockSync {
    /// `t0` = local time sent, `hub_ms` = hub time in the answer, `t3` = local
    /// time the answer arrived.
    pub fn add(&mut self, t0: u64, hub_ms: u64, t3: u64) {
        let Some(rtt) = t3.checked_sub(t0) else { return };
        // the hub read its clock about half way through the round trip
        let offset = hub_ms as i64 - (t0 + rtt / 2) as i64;
        if self.samples.len() == SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back((rtt, offset));
    }

    /// `hub - local` in milliseconds; `None` before the first answer.
    pub fn offset(&self) -> Option<i64> {
        self.samples.iter().min_by_key(|(rtt, _)| *rtt).map(|(_, o)| *o)
    }

    /// Round trip of the sample in use (how precise the offset is).
    pub fn round_trip(&self) -> Option<u64> {
        self.samples.iter().map(|(rtt, _)| *rtt).min()
    }

    /// The hub's clock now, given the local clock.
    pub fn hub_now(&self, local_ms: u64) -> Option<u64> {
        self.offset().map(|o| (local_ms as i64 + o).max(0) as u64)
    }
}

/// Where the hub's song is at `hub_now` (from a `state` measured at `clock_ms`).
pub fn expected_position(position_ms: u64, clock_ms: u64, playing: bool, hub_now: u64) -> u64 {
    if playing {
        position_ms + hub_now.saturating_sub(clock_ms)
    } else {
        position_ms
    }
}

/// Further apart than this: jump straight there.
pub const SEEK_OVER_MS: i64 = 350;
/// Closer than this: in sync, nobody hears it.
pub const IN_SYNC_MS: i64 = 15;
/// A small gap is closed by playing a little faster or slower over about this long.
const CATCH_UP_MS: f32 = 3000.0;
/// Speed changes stay within ±2 %: time-stretching more than that can be heard on music.
const MAX_RATE_CHANGE: f32 = 0.02;

/// What a guest's player should do to stay with the hub.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SyncAction {
    /// Play at normal speed.
    Keep,
    /// Play at this speed for a while (1.0 = normal).
    Rate(f32),
    /// Jump to this position (milliseconds).
    Seek(u64),
}

/// `target_ms` = where the player should be now (the hub's position plus this
/// speaker's delay), `actual_ms` = where it is.
pub fn sync_action(target_ms: u64, actual_ms: u64) -> SyncAction {
    let behind = target_ms as i64 - actual_ms as i64;
    if behind.abs() > SEEK_OVER_MS {
        SyncAction::Seek(target_ms)
    } else if behind.abs() <= IN_SYNC_MS {
        SyncAction::Keep
    } else {
        let change = (behind as f32 / CATCH_UP_MS).clamp(-MAX_RATE_CHANGE, MAX_RATE_CHANGE);
        SyncAction::Rate(1.0 + change)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fastest_round_trip_sets_the_offset() {
        let mut c = ClockSync::default();
        assert_eq!(c.offset(), None);
        // hub is 1000 ms ahead; a slow sample with a lopsided trip is off by 40 ms
        c.add(100, 1190, 200); // rtt 100, says 1040
        c.add(300, 1310, 320); // rtt 20, says 1000
        c.add(400, 1450, 480); // rtt 80, says 1010
        assert_eq!(c.offset(), Some(1000));
        assert_eq!(c.round_trip(), Some(20));
        assert_eq!(c.hub_now(5000), Some(6000));
        // answers from before the request are ignored
        c.add(500, 1, 400);
        assert_eq!(c.samples.len(), 3);
    }

    #[test]
    fn old_samples_fall_out() {
        let mut c = ClockSync::default();
        c.add(0, 500, 2); // best, then pushed out
        for i in 0..SAMPLES as u64 {
            c.add(100 + i * 10, 700 + i * 10, 110 + i * 10);
        }
        assert_eq!(c.offset(), Some(595));
    }

    #[test]
    fn expected_position_moves_only_while_playing() {
        assert_eq!(expected_position(10_000, 50_000, true, 52_500), 12_500);
        assert_eq!(expected_position(10_000, 50_000, false, 52_500), 10_000);
        // a state from the future (clock jitter) never moves it backwards
        assert_eq!(expected_position(10_000, 50_000, true, 49_000), 10_000);
    }

    #[test]
    fn small_gaps_change_speed_big_ones_seek() {
        assert_eq!(sync_action(10_000, 10_005), SyncAction::Keep);
        assert_eq!(sync_action(10_000, 9_000), SyncAction::Seek(10_000));
        assert_eq!(sync_action(10_000, 10_500), SyncAction::Seek(10_000));
        let SyncAction::Rate(r) = sync_action(10_000, 9_940) else { panic!("rate") };
        assert!((r - 1.02).abs() < 1e-4, "{r}");
        let SyncAction::Rate(r) = sync_action(10_000, 10_150) else { panic!("rate") };
        assert!((r - 0.98).abs() < 1e-4, "{r}");
        // a gap a jump would make worse than it is: still by speed
        assert!(matches!(sync_action(10_000, 9_700), SyncAction::Rate(_)));
    }
}
