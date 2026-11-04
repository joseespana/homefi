//! "Get ready for a concert": the songs an artist plays live on this tour.
//!
//! This module holds the shared types, the pure tour set ([`tour_set`]),
//! Tidal's most played ([`most_played`]) and the words of the feature. The AI
//! search and the build cascade sit on top of it.

use serde::{Deserialize, Serialize};

mod ai_search;
mod build;
mod popular;
mod tour;
pub(crate) mod words;

pub use build::{build, rearrange, BuildContext, ConcertBuilt, ConcertProgress, BUILD_DEADLINE};
pub use popular::{most_played, POPULAR_LIMIT};
pub use tour::{show_set, tour_set, TourSet, TourSong};

pub const MIN_CONCERT_SHOWS: u8 = 3;
pub const MAX_CONCERT_SHOWS: u8 = 15;
pub const DEFAULT_CONCERT_SHOWS: u8 = 8;
pub const MIN_THRESHOLD_PCT: u8 = 5;
pub const MAX_THRESHOLD_PCT: u8 = 100;
/// Tour mode needs at least this many usable shows, each with at least
/// [`MIN_SONGS_PER_SHOW`] songs that aren't tapes; otherwise the chain falls
/// back to Tidal's most played.
pub const MIN_TOUR_SHOWS: usize = 2;
pub const MIN_SONGS_PER_SHOW: usize = 5;

const DEFAULT_ALWAYS_PCT: u8 = 80;
const DEFAULT_USUALLY_PCT: u8 = 50;
const DEFAULT_SOMETIMES_PCT: u8 = 25;
const DEFAULT_ENCORE_PCT: u8 = 50;

/// How often a song must be played (share of the shows) to get each tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConcertThresholds {
    pub always_pct: u8,
    pub usually_pct: u8,
    pub sometimes_pct: u8,
    /// A song is "Encore" when at least this share of its plays were in the encore.
    pub encore_pct: u8,
}

impl Default for ConcertThresholds {
    fn default() -> Self {
        Self {
            always_pct: DEFAULT_ALWAYS_PCT,
            usually_pct: DEFAULT_USUALLY_PCT,
            sometimes_pct: DEFAULT_SOMETIMES_PCT,
            encore_pct: DEFAULT_ENCORE_PCT,
        }
    }
}

impl ConcertThresholds {
    /// Each value into 5..=100, then usually <= always and sometimes <= usually
    /// (lowered to fit). `encore_pct` is independent.
    pub fn clamped(self) -> Self {
        let pct = |v: u8| v.clamp(MIN_THRESHOLD_PCT, MAX_THRESHOLD_PCT);
        let always_pct = pct(self.always_pct);
        let usually_pct = pct(self.usually_pct).min(always_pct);
        let sometimes_pct = pct(self.sometimes_pct).min(usually_pct);
        Self { always_pct, usually_pct, sometimes_pct, encore_pct: pct(self.encore_pct) }
    }
}

/// One song of a show's setlist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ConcertSong {
    pub name: String,
    /// The original artist when it's a cover.
    pub original_artist: Option<String>,
    /// A recorded intro or outro, not played live.
    pub tape: bool,
    pub encore: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ConcertShow {
    /// `<date>-<n>`, given by the core, unique within a playlist.
    pub id: String,
    /// ISO `yyyy-mm-dd`.
    pub date: String,
    pub venue: String,
    pub city: String,
    pub country: String,
    pub tour: String,
    /// The page it was read from, when the AI gave one.
    pub url: Option<String>,
    pub songs: Vec<ConcertSong>,
}

/// Which setlist the playlist is made from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConcertMode {
    /// What the artist plays most nights of the tour.
    #[default]
    Tour,
    /// One concert, in its own order.
    Show { show_id: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConcertRequest {
    pub artist: String,
    /// Recent shows to consider (clamped 3..=15).
    pub shows: u8,
    pub thresholds: ConcertThresholds,
    pub mode: ConcertMode,
    /// Skip the AI: Tidal's most played.
    pub popular_only: bool,
}

impl Default for ConcertRequest {
    fn default() -> Self {
        Self {
            artist: String::new(),
            shows: DEFAULT_CONCERT_SHOWS,
            thresholds: ConcertThresholds::default(),
            mode: ConcertMode::default(),
            popular_only: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(a: u8, u: u8, s: u8, e: u8) -> ConcertThresholds {
        ConcertThresholds { always_pct: a, usually_pct: u, sometimes_pct: s, encore_pct: e }
    }

    #[test]
    fn thresholds_clamp() {
        assert_eq!(t(80, 50, 25, 50).clamped(), t(80, 50, 25, 50));
        assert_eq!(t(60, 90, 70, 50).clamped(), t(60, 60, 60, 50));
        assert_eq!(t(0, 200, 1, 0).clamped(), t(5, 5, 5, 5));
        assert_eq!(t(100, 100, 100, 100).clamped(), t(100, 100, 100, 100));
        assert_eq!(ConcertThresholds::default(), t(80, 50, 25, 50));
    }

    #[test]
    fn request_defaults_and_partial_json() {
        let r: ConcertRequest = serde_json::from_str(r#"{"artist":"Korn"}"#).unwrap();
        assert_eq!((r.shows, r.popular_only), (DEFAULT_CONCERT_SHOWS, false));
        assert_eq!(r.mode, ConcertMode::Tour);
        let m: ConcertMode = serde_json::from_str(r#"{"kind":"show","show_id":"2026-10-24-0"}"#).unwrap();
        assert_eq!(m, ConcertMode::Show { show_id: "2026-10-24-0".into() });
    }
}
