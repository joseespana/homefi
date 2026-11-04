//! The pure part of the build: how many songs fit the length, how many per
//! artist, and in which order they play. No I/O, no AI.

use std::collections::HashMap;

use super::words::{self, Phase};
use super::EnergyCurve;
use crate::humane::Lang;
use crate::playlist::{artist_key, EntryTag, PlaylistEntry};

/// What a song with no known duration counts as, and the average song when planning the length.
pub(crate) const AVG_SONG_MS: u64 = 225_000;
/// Where the loudest song of an arc sits, as a share of the list.
const PEAK_AT_PERCENT: usize = 65;
const PERCENT: usize = 100;
/// Share of the list that is the warm-up and the wind-down of an arc.
const WARM_UP_PERCENT: usize = 25;
const WIND_DOWN_PERCENT: usize = 20;

/// A song found, with the energy (1..=5) the AI gave it.
#[derive(Debug, Clone, PartialEq)]
pub struct Picked {
    pub entry: PlaylistEntry,
    /// 1..=5.
    pub energy: u8,
}

/// How long a song counts for: its duration, or [`AVG_SONG_MS`] when unknown.
pub(crate) fn effective_ms(entry: &PlaylistEntry) -> u64 {
    if entry.duration_ms == 0 {
        AVG_SONG_MS
    } else {
        entry.duration_ms
    }
}

/// Keeps the first `max` songs of each artist (by main artist), order kept.
pub fn cap_per_artist(songs: Vec<Picked>, max: u8) -> Vec<Picked> {
    let mut counts: HashMap<String, u8> = HashMap::new();
    songs
        .into_iter()
        .filter(|song| {
            let count = counts.entry(artist_key(&song.entry.artist)).or_default();
            *count += 1;
            *count <= max
        })
        .collect()
}

/// `(picked, rest)`: in order, songs are added while the total is under
/// `target - AVG/2`, so the total lands within half an average song of the
/// target, and never above `target` plus the longest picked song.
pub fn pick_for_duration(songs: Vec<Picked>, target_ms: u64) -> (Vec<Picked>, Vec<Picked>) {
    let goal_ms = target_ms.saturating_sub(AVG_SONG_MS / 2);
    let mut total_ms = 0u64;
    let mut picked = Vec::new();
    let mut rest = Vec::new();
    for song in songs {
        if rest.is_empty() && total_ms < goal_ms {
            total_ms += effective_ms(&song.entry);
            picked.push(song);
        } else {
            rest.push(song);
        }
    }
    (picked, rest)
}

/// Puts the songs in playing order and fills `tag` and `note` of each entry.
pub fn arrange(songs: Vec<Picked>, curve: EnergyCurve, lang: Lang) -> Vec<PlaylistEntry> {
    let (ordered, notes) = match curve {
        EnergyCurve::Arc => {
            let ordered = arc_order(songs);
            let notes = arc_notes(ordered.len(), lang);
            (ordered, notes)
        }
        EnergyCurve::Rising => {
            let mut ordered = songs;
            ordered.sort_by_key(|s| s.energy);
            let notes = vec![String::new(); ordered.len()];
            (ordered, notes)
        }
        EnergyCurve::Steady => {
            let notes = vec![String::new(); songs.len()];
            (songs, notes)
        }
    };
    ordered
        .into_iter()
        .zip(notes)
        .map(|(song, note)| PlaylistEntry { tag: EntryTag::Plain, note, ..song.entry })
        .collect()
}

/// Low energy at both ends, the highest around [`PEAK_AT_PERCENT`] of the list:
/// the loudest songs are placed outward from the peak, one side then the other.
fn arc_order(mut songs: Vec<Picked>) -> Vec<Picked> {
    let count = songs.len();
    if count < 2 {
        return songs;
    }
    // stable: equal energies keep the AI's order
    songs.sort_by_key(|s| std::cmp::Reverse(s.energy));
    let last = count - 1;
    let peak_at = (last * PEAK_AT_PERCENT + PERCENT / 2) / PERCENT;
    let (mut before, mut after): (Vec<Picked>, Vec<Picked>) = (Vec::new(), Vec::new());
    let mut songs = songs.into_iter();
    let Some(peak) = songs.next() else { return Vec::new() };
    let after_capacity = last - peak_at;
    for (i, song) in songs.enumerate() {
        let before_turn = i % 2 == 0;
        if (before_turn && before.len() < peak_at) || after.len() >= after_capacity {
            before.push(song);
        } else {
            after.push(song);
        }
    }
    before.reverse();
    before.push(peak);
    before.extend(after);
    before
}

fn arc_notes(count: usize, lang: Lang) -> Vec<String> {
    let warm_up = count * WARM_UP_PERCENT / PERCENT;
    let wind_down_from = count - count * WIND_DOWN_PERCENT / PERCENT;
    (0..count)
        .map(|i| {
            let phase = if i < warm_up {
                Phase::WarmUp
            } else if i >= wind_down_from {
                Phase::WindDown
            } else {
                Phase::Peak
            };
            words::phase(lang, phase)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(title: &str, artist: &str, energy: u8, duration_ms: u64) -> Picked {
        Picked {
            entry: PlaylistEntry { title: title.into(), artist: artist.into(), duration_ms, ..Default::default() },
            energy,
        }
    }

    /// `n` songs by distinct artists with a spread of energies, in a fixed pseudo-random order.
    fn spread(n: usize) -> Vec<Picked> {
        (0..n)
            .map(|i| song(&format!("s{i}"), &format!("a{i}"), u8::try_from((i * 7 + 3) % 5).unwrap() + 1, AVG_SONG_MS))
            .collect()
    }

    fn energies(entries: &[PlaylistEntry], songs: &[Picked]) -> Vec<u8> {
        entries.iter().map(|e| songs.iter().find(|s| s.entry.title == e.title).map(|s| s.energy).unwrap()).collect()
    }

    #[test]
    fn an_arc_peaks_between_half_and_four_fifths_and_starts_and_ends_calm() {
        for n in [5usize, 12, 40] {
            let songs = spread(n);
            let entries = arrange(songs.clone(), EnergyCurve::Arc, Lang::En);
            assert_eq!(entries.len(), n);
            let e = energies(&entries, &songs);
            let max = *e.iter().max().unwrap();
            let last = n - 1;
            let peak = e.iter().position(|&x| x == max).unwrap();
            assert!(peak * 10 >= last * 5 && peak * 10 <= last * 8, "n={n}: peak at {peak} of {last}");
            let mut sorted = e.clone();
            sorted.sort_unstable();
            let median = sorted[n / 2];
            assert!(e[0] <= median && e[last] <= median, "n={n}: ends {} {} median {median}", e[0], e[last]);
            assert!(entries.iter().all(|x| x.tag == EntryTag::Plain));
        }
    }

    #[test]
    fn an_arc_keeps_every_song_once() {
        let songs = spread(17);
        let mut titles: Vec<String> =
            arrange(songs.clone(), EnergyCurve::Arc, Lang::En).into_iter().map(|e| e.title).collect();
        titles.sort();
        let mut expected: Vec<String> = songs.into_iter().map(|s| s.entry.title).collect();
        expected.sort();
        assert_eq!(titles, expected);
    }

    #[test]
    fn equal_energies_keep_the_ai_order_in_a_rising_curve() {
        let songs = vec![song("a", "x", 3, 0), song("b", "y", 1, 0), song("c", "z", 3, 0), song("d", "w", 1, 0)];
        let titles: Vec<String> = arrange(songs, EnergyCurve::Rising, Lang::En).into_iter().map(|e| e.title).collect();
        assert_eq!(titles, ["b", "d", "a", "c"]);
    }

    #[test]
    fn steady_keeps_the_order_and_has_no_notes() {
        let songs = spread(9);
        let entries = arrange(songs.clone(), EnergyCurve::Steady, Lang::En);
        let expected: Vec<&str> = songs.iter().map(|s| s.entry.title.as_str()).collect();
        assert_eq!(entries.iter().map(|e| e.title.as_str()).collect::<Vec<_>>(), expected);
        assert!(entries.iter().all(|e| e.note.is_empty()));
        let rising = arrange(songs, EnergyCurve::Rising, Lang::En);
        assert!(rising.iter().all(|e| e.note.is_empty()));
    }

    #[test]
    fn arc_phases_follow_the_shares() {
        let entries = arrange(spread(40), EnergyCurve::Arc, Lang::En);
        let count = |word: &str| entries.iter().filter(|e| e.note == word).count();
        assert_eq!((count("Warm-up"), count("Peak"), count("Wind-down")), (10, 22, 8));
        assert_eq!(entries[0].note, "Warm-up");
        assert_eq!(entries[39].note, "Wind-down");
        let es = arrange(spread(10), EnergyCurve::Arc, Lang::Es);
        assert_eq!(es[0].note, "Para calentar");
        assert_eq!(es[9].note, "Para cerrar");
    }

    #[test]
    fn tiny_lists_do_not_panic() {
        for n in 0..4 {
            assert_eq!(arrange(spread(n), EnergyCurve::Arc, Lang::En).len(), n);
        }
    }

    fn total(songs: &[Picked]) -> u64 {
        songs.iter().map(|s| effective_ms(&s.entry)).sum()
    }

    #[test]
    fn picking_for_a_duration_stays_close() {
        let songs: Vec<Picked> = (0..10).map(|i| song(&format!("s{i}"), &format!("a{i}"), 3, AVG_SONG_MS)).collect();
        // exact fit: 4 songs = 900 s, the 4th lands at 3.5 AVG... stop once the total reaches target - AVG/2
        let (picked, rest) = pick_for_duration(songs.clone(), 4 * AVG_SONG_MS);
        assert_eq!((picked.len(), rest.len()), (4, 6));
        assert!(total(&picked) <= 4 * AVG_SONG_MS + AVG_SONG_MS);
        // order is kept
        assert_eq!(picked[0].entry.title, "s0");
        assert_eq!(rest[0].entry.title, "s4");
    }

    #[test]
    fn zero_durations_count_as_an_average_song() {
        let songs: Vec<Picked> = (0..10).map(|i| song(&format!("s{i}"), &format!("a{i}"), 3, 0)).collect();
        let (picked, _) = pick_for_duration(songs, 3 * AVG_SONG_MS);
        assert_eq!(picked.len(), 3);
    }

    #[test]
    fn one_very_long_song_ends_the_picking() {
        let songs = vec![song("long", "a", 3, 3_600_000), song("b", "b", 3, AVG_SONG_MS)];
        let target = 30 * 60_000;
        let (picked, rest) = pick_for_duration(songs, target);
        assert_eq!((picked.len(), rest.len()), (1, 1));
        assert!(total(&picked) <= target + 3_600_000);
    }

    #[test]
    fn picking_from_nothing_or_for_nothing() {
        assert_eq!(pick_for_duration(Vec::new(), 1_000_000), (Vec::new(), Vec::new()));
        let (picked, rest) = pick_for_duration(spread(3), 0);
        assert!(picked.is_empty());
        assert_eq!(rest.len(), 3);
    }

    #[test]
    fn the_cap_counts_the_main_artist_of_a_credit() {
        let songs = vec![
            song("1", "Bad Bunny", 3, 0),
            song("2", "Bad Bunny feat. Drake", 3, 0),
            song("3", "Drake", 3, 0),
            song("4", "Bad Bunny & Jhay Cortez", 3, 0),
            song("5", "Bad Bunny", 3, 0),
        ];
        let kept: Vec<String> = cap_per_artist(songs, 2).into_iter().map(|s| s.entry.title).collect();
        assert_eq!(kept, ["1", "2", "3"]);
    }
}
