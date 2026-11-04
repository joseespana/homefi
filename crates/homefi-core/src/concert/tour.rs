//! The tour set: which songs an artist plays most nights, tagged and ordered.
//! Pure (no network, no clock), so the thresholds are easy to test.

use std::collections::HashMap;

use super::{ConcertShow, ConcertThresholds};
use crate::playlist::EntryTag;
use crate::text::song_key;

const PERCENT: usize = 100;

/// One song of the tour, with how often and where it is played.
#[derive(Debug, Clone, PartialEq)]
pub struct TourSong {
    /// Spelling from the most recent show that has it.
    pub name: String,
    pub original_artist: Option<String>,
    /// Flagged a tape in most of its plays.
    pub tape: bool,
    pub plays: usize,
    /// `round(100 * plays / shows_considered)`.
    pub share_pct: u8,
    /// Median relative position in the show, 0..=1.
    pub median_pos: f32,
    /// `round(100 * encore plays / plays)`.
    pub encore_share_pct: u8,
    pub tag: EntryTag,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TourSet {
    pub songs: Vec<TourSong>,
    pub shows_considered: usize,
}

/// A song in one show: its key, relative position and the song itself.
struct Played<'a> {
    key: String,
    pos: f32,
    song: &'a super::ConcertSong,
}

/// The show's songs without repeats (first play wins, empty keys ignored),
/// with their relative position.
fn played(show: &ConcertShow) -> Vec<Played<'_>> {
    let mut seen = std::collections::HashSet::new();
    let kept: Vec<_> = show
        .songs
        .iter()
        .filter_map(|song| {
            let key = song_key(&song.name);
            (!key.is_empty() && seen.insert(key.clone())).then_some((key, song))
        })
        .collect();
    let span = kept.len().saturating_sub(1).max(1) as f32;
    kept.into_iter().enumerate().map(|(i, (key, song))| Played { key, pos: i as f32 / span, song }).collect()
}

fn percent(part: usize, whole: usize) -> u8 {
    if whole == 0 {
        return 0;
    }
    u8::try_from((PERCENT * part + whole / 2) / whole).unwrap_or(u8::MAX)
}

fn median(values: &mut [f32]) -> f32 {
    values.sort_by(f32::total_cmp);
    let mid = values.len() / 2;
    match values.len() {
        0 => 0.0,
        n if n % 2 == 1 => values[mid],
        _ => (values[mid - 1] + values[mid]) / 2.0,
    }
}

#[derive(Default)]
struct Agg {
    name: String,
    original_artist: Option<String>,
    plays: usize,
    tape_plays: usize,
    encore_plays: usize,
    positions: Vec<f32>,
}

/// The songs played in the given shows (most recent first), tagged by
/// `thresholds` and ordered by their median position, encores last. Songs
/// played in fewer shows than `sometimes_pct` are left out. Tapes stay in the
/// set with `tape: true`; the caller drops them if it wants.
pub fn tour_set(shows: &[ConcertShow], thresholds: &ConcertThresholds) -> TourSet {
    let t = thresholds.clamped();
    let usable: Vec<&ConcertShow> = shows.iter().filter(|s| s.songs.iter().any(|song| !song.tape)).collect();
    let considered = usable.len();

    let mut order: Vec<String> = Vec::new();
    let mut aggs: HashMap<String, Agg> = HashMap::new();
    for show in &usable {
        for p in played(show) {
            let agg = aggs.entry(p.key.clone()).or_insert_with(|| {
                order.push(p.key.clone());
                Agg { name: p.song.name.trim().to_owned(), ..Agg::default() }
            });
            agg.plays += 1;
            agg.tape_plays += usize::from(p.song.tape);
            agg.encore_plays += usize::from(p.song.encore);
            agg.positions.push(p.pos);
            if agg.original_artist.is_none() {
                agg.original_artist =
                    p.song.original_artist.as_deref().map(str::trim).filter(|a| !a.is_empty()).map(str::to_owned);
            }
        }
    }

    let mut songs: Vec<TourSong> = order
        .iter()
        .filter_map(|key| {
            let mut agg = aggs.remove(key)?;
            let share_pct = percent(agg.plays, considered);
            let tag = if share_pct >= t.always_pct {
                EntryTag::Always
            } else if share_pct >= t.usually_pct {
                EntryTag::Usually
            } else if share_pct >= t.sometimes_pct {
                EntryTag::Sometimes
            } else {
                return None;
            };
            let encore_share_pct = percent(agg.encore_plays, agg.plays);
            Some(TourSong {
                name: agg.name,
                original_artist: agg.original_artist,
                tape: agg.tape_plays * 2 > agg.plays,
                plays: agg.plays,
                share_pct,
                median_pos: median(&mut agg.positions),
                encore_share_pct,
                tag: if encore_share_pct >= t.encore_pct { EntryTag::Encore } else { tag },
            })
        })
        .collect();

    songs.sort_by(|a, b| {
        let encore = |s: &TourSong| s.tag == EntryTag::Encore;
        encore(a)
            .cmp(&encore(b))
            .then(a.median_pos.total_cmp(&b.median_pos))
            .then(b.plays.cmp(&a.plays))
            .then_with(|| a.name.cmp(&b.name))
    });
    TourSet { songs, shows_considered: considered }
}

/// One show as played: literal order, no repeats; encore songs tagged Encore,
/// the rest Always.
pub fn show_set(show: &ConcertShow) -> TourSet {
    let songs = played(show)
        .into_iter()
        .map(|p| TourSong {
            name: p.song.name.trim().to_owned(),
            original_artist: p
                .song
                .original_artist
                .as_deref()
                .map(str::trim)
                .filter(|a| !a.is_empty())
                .map(str::to_owned),
            tape: p.song.tape,
            plays: 1,
            share_pct: PERCENT as u8,
            median_pos: p.pos,
            encore_share_pct: if p.song.encore { PERCENT as u8 } else { 0 },
            tag: if p.song.encore { EntryTag::Encore } else { EntryTag::Always },
        })
        .collect();
    TourSet { songs, shows_considered: 1 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::concert::ConcertSong;

    fn song(name: &str) -> ConcertSong {
        ConcertSong { name: name.into(), ..ConcertSong::default() }
    }

    fn encore(name: &str) -> ConcertSong {
        ConcertSong { name: name.into(), encore: true, ..ConcertSong::default() }
    }

    fn tape(name: &str) -> ConcertSong {
        ConcertSong { name: name.into(), tape: true, ..ConcertSong::default() }
    }

    fn show(n: usize, songs: Vec<ConcertSong>) -> ConcertShow {
        ConcertShow { id: format!("2026-10-{n:02}-0"), songs, ..ConcertShow::default() }
    }

    fn find<'a>(set: &'a TourSet, name: &str) -> &'a TourSong {
        set.songs.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("{name} is missing"))
    }

    fn names(set: &TourSet) -> Vec<&str> {
        set.songs.iter().map(|s| s.name.as_str()).collect()
    }

    /// 8 shows: Blind 8/8, Twist 6/8, Falling Away 4/8, Coming Undone 2/8, Rare 1/8.
    fn eight_shows() -> Vec<ConcertShow> {
        (0..8)
            .map(|i| {
                let mut songs = vec![song("Blind")];
                if i < 6 {
                    songs.push(song("Twist"));
                }
                if i < 4 {
                    songs.push(song("Falling Away"));
                }
                if i < 2 {
                    songs.push(song("Coming Undone"));
                }
                if i == 0 {
                    songs.push(song("Rare"));
                }
                songs
            })
            .enumerate()
            .map(|(i, songs)| show(i, songs))
            .collect()
    }

    #[test]
    fn tags_at_their_boundaries() {
        let set = tour_set(&eight_shows(), &ConcertThresholds::default());
        assert_eq!(set.shows_considered, 8);
        assert_eq!(find(&set, "Blind").tag, EntryTag::Always);
        assert_eq!(find(&set, "Blind").share_pct, 100);
        assert_eq!(find(&set, "Twist").tag, EntryTag::Usually); // 75 %
        assert_eq!(find(&set, "Falling Away").tag, EntryTag::Usually); // 4/8 = 50 %, inclusive
        assert_eq!(find(&set, "Coming Undone").tag, EntryTag::Sometimes); // 2/8 = 25 %, inclusive
        assert!(!names(&set).contains(&"Rare")); // 1/8 = 13 %
    }

    #[test]
    fn custom_thresholds_change_the_tags() {
        let strict = ConcertThresholds { always_pct: 100, ..ConcertThresholds::default() };
        let set = tour_set(&eight_shows(), &strict);
        let always = set.songs.iter().filter(|s| s.tag == EntryTag::Always).count();
        assert_eq!(always, 1);
        let loose = ConcertThresholds { always_pct: 70, ..ConcertThresholds::default() };
        let always = tour_set(&eight_shows(), &loose).songs.iter().filter(|s| s.tag == EntryTag::Always).count();
        assert_eq!(always, 2); // Blind and Twist (75 %)
    }

    #[test]
    fn ordered_by_median_position() {
        let shows = vec![
            show(1, vec![song("A"), song("B"), song("C")]),
            show(2, vec![song("A"), song("C"), song("B")]),
            show(3, vec![song("B"), song("A"), song("C")]),
        ];
        let set = tour_set(&shows, &ConcertThresholds::default());
        // A: 0, 0, .5 -> 0 ; B: .5, 1, 0 -> .5 ; C: 1, .5, 1 -> 1
        assert_eq!(names(&set), vec!["A", "B", "C"]);
        assert!((find(&set, "B").median_pos - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn encore_songs_go_last_in_order() {
        let shows: Vec<_> = (0..4)
            .map(|i| show(i, vec![song("Opener"), encore("Bis One"), encore("Bis Two"), song("Middle")]))
            .collect();
        let set = tour_set(&shows, &ConcertThresholds::default());
        assert_eq!(names(&set), vec!["Opener", "Middle", "Bis One", "Bis Two"]);
        assert_eq!(find(&set, "Bis One").tag, EntryTag::Encore);
        assert_eq!(find(&set, "Bis One").encore_share_pct, 100);
    }

    #[test]
    fn encore_in_half_of_its_plays_is_encore() {
        let shows: Vec<_> = (0..6)
            .map(|i| show(i, vec![song("Opener"), if i < 3 { encore("Freak") } else { song("Freak") }]))
            .collect();
        let set = tour_set(&shows, &ConcertThresholds::default());
        assert_eq!(find(&set, "Freak").plays, 6);
        assert_eq!(find(&set, "Freak").encore_share_pct, 50);
        assert_eq!(find(&set, "Freak").tag, EntryTag::Encore);
        let picky = ConcertThresholds { encore_pct: 51, ..ConcertThresholds::default() };
        assert_eq!(find(&tour_set(&shows, &picky), "Freak").tag, EntryTag::Always);
    }

    #[test]
    fn tapes_are_flagged_and_kept() {
        let shows: Vec<_> = (0..4).map(|i| show(i, vec![tape("Intro"), song("Blind")])).collect();
        let set = tour_set(&shows, &ConcertThresholds::default());
        assert!(find(&set, "Intro").tape);
        assert!(!find(&set, "Blind").tape);
    }

    #[test]
    fn covers_keep_the_original_artist() {
        let cover = ConcertSong {
            name: "Wicked Game".into(),
            original_artist: Some("Chris Isaak".into()),
            ..ConcertSong::default()
        };
        let shows = vec![show(1, vec![song("A"), cover.clone()]), show(2, vec![song("A"), song("Wicked Game")])];
        let set = tour_set(&shows, &ConcertThresholds::default());
        assert_eq!(find(&set, "Wicked Game").original_artist.as_deref(), Some("Chris Isaak"));
    }

    #[test]
    fn a_song_repeated_in_one_show_counts_once() {
        let shows = vec![show(1, vec![song("Blind"), song("Other"), song("Blind")])];
        let set = tour_set(&shows, &ConcertThresholds::default());
        assert_eq!(find(&set, "Blind").plays, 1);
        assert_eq!(set.songs.len(), 2);
    }

    #[test]
    fn spelling_variants_merge_and_keep_the_latest_spelling() {
        let shows = vec![
            show(1, vec![song("Freak on a Leash"), song("X")]),
            show(2, vec![song("Freak On A Leash (Live)"), song("X")]),
            show(3, vec![song("Freak On A Leash - Live"), song("X")]),
        ];
        let set = tour_set(&shows, &ConcertThresholds::default());
        assert_eq!(find(&set, "Freak on a Leash").plays, 3);
        assert_eq!(set.songs.len(), 2);
    }

    #[test]
    fn a_show_with_only_tapes_is_not_counted() {
        let shows = vec![show(1, vec![tape("Intro"), tape("Outro")]), show(2, vec![song("Blind")])];
        let set = tour_set(&shows, &ConcertThresholds::default());
        assert_eq!(set.shows_considered, 1);
        assert_eq!(find(&set, "Blind").share_pct, 100);
        assert!(!names(&set).contains(&"Intro"));
    }

    #[test]
    fn no_shows_gives_an_empty_set() {
        let set = tour_set(&[], &ConcertThresholds::default());
        assert!(set.songs.is_empty());
        assert_eq!(set.shows_considered, 0);
    }

    #[test]
    fn show_set_keeps_the_literal_order() {
        let s = show(1, vec![tape("Intro"), song("Blind"), song("Twist"), song("blind"), encore("Bis")]);
        let set = show_set(&s);
        assert_eq!(names(&set), vec!["Intro", "Blind", "Twist", "Bis"]);
        assert_eq!(set.shows_considered, 1);
        assert_eq!(find(&set, "Blind").tag, EntryTag::Always);
        assert_eq!(find(&set, "Bis").tag, EntryTag::Encore);
        assert_eq!(find(&set, "Twist").share_pct, 100);
        assert!(find(&set, "Intro").tape);
    }
}
