//! The playlist file: what is saved in `<app dir>/playlists/<id>.json`. Read
//! leniently: missing fields take their defaults, unknown fields are ignored
//! and an unknown `tag` reads as `plain`.

use serde::{Deserialize, Serialize};

use crate::concert::{ConcertMode, ConcertShow, ConcertThresholds};
use crate::dsp::taste::SourceContext;
use crate::exercise::MusicWish;
use crate::party::{ChatBrief, PartyBrief};

pub const PLAYLIST_FORMAT_VERSION: u32 = 1;
/// Folder of the playlists inside the app folder.
pub const PLAYLISTS_DIR: &str = "playlists";
/// Most pages kept as sources of one playlist.
pub const MAX_SOURCES: usize = 20;

/// Mask for the 12-bit counter that keeps ids made in the same millisecond apart.
const ID_COUNTER_MASK: u32 = 0xfff;
/// `SourceContext` kind that makes the Sounds of a playlist apply.
const PLAYLIST_KIND: &str = "playlist";

/// How often a song is played, shown as a chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EntryTag {
    Always,
    Usually,
    Sometimes,
    Encore,
    #[default]
    #[serde(other)]
    Plain,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PlaylistEntry {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// As `catalog::Hit::id`.
    pub tidal_id: Option<String>,
    /// Upper case.
    pub isrc: Option<String>,
    /// Absolute path of a library file.
    pub local_path: Option<String>,
    pub duration_ms: u64,
    pub explicit: bool,
    pub cover_url: Option<String>,
    pub tag: EntryTag,
    /// "Played in 7 of 8 shows · encore", in the build language.
    pub note: String,
}

/// A song that was asked for, before it is matched to a track.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SongCandidate {
    pub title: String,
    pub artist: String,
    pub tag: EntryTag,
    pub note: String,
}

/// A page the AI read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SourceLink {
    pub url: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SetlistSource {
    Ai {
        provider_id: String,
        model: String,
    },
    #[default]
    TidalPopular,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ConcertOrigin {
    pub artist: String,
    pub source: SetlistSource,
    pub mode: ConcertMode,
    pub shows_wanted: u8,
    pub thresholds: ConcertThresholds,
    /// Every show found, most recent first (empty for Tidal's most played).
    pub shows: Vec<ConcertShow>,
    pub fetched_ms: u64,
}

/// A party playlist: the answers it was built from, so "Change answers" can start from them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PartyOrigin {
    pub brief: PartyBrief,
    pub provider_id: String,
    pub model: String,
    pub built_ms: u64,
}

/// A chat ("Playlist with AI") playlist: what was asked, so "Keep chatting" can start from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ChatOrigin {
    pub brief: ChatBrief,
    pub provider_id: String,
    pub model: String,
    pub built_ms: u64,
}

/// An Exercise ride: the workout and the music wish it was built from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RideOrigin {
    /// `Workout::id` of the ride's plan.
    pub workout_id: String,
    pub wish: MusicWish,
}

/// How the playlist was made. Other kinds can be added later.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlaylistOrigin {
    Concert(ConcertOrigin),
    Party(PartyOrigin),
    Chat(ChatOrigin),
    Ride(RideOrigin),
}

impl Default for PlaylistOrigin {
    fn default() -> Self {
        Self::Concert(ConcertOrigin::default())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HomePlaylist {
    pub version: u32,
    /// `[A-Za-z0-9_-]{1,64}`, see [`new_id`].
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_ms: u64,
    pub updated_ms: u64,
    pub origin: PlaylistOrigin,
    pub entries: Vec<PlaylistEntry>,
    /// Songs that couldn't be matched with enough confidence.
    pub unmatched: Vec<SongCandidate>,
    /// At most [`MAX_SOURCES`], deduplicated by URL.
    pub sources: Vec<SourceLink>,
    /// Google's Search Suggestions HTML, shown with a grounded result.
    pub search_widget_html: Option<String>,
    /// The copy saved in the user's TIDAL account (see [`super::save_to_tidal`]); `None`
    /// until it is saved there.
    pub tidal_uuid: Option<String>,
}

impl Default for HomePlaylist {
    fn default() -> Self {
        Self {
            version: PLAYLIST_FORMAT_VERSION,
            id: String::new(),
            name: String::new(),
            description: String::new(),
            created_ms: 0,
            updated_ms: 0,
            origin: PlaylistOrigin::default(),
            entries: Vec::new(),
            unmatched: Vec::new(),
            sources: Vec::new(),
            search_widget_html: None,
            tidal_uuid: None,
        }
    }
}

impl HomePlaylist {
    pub fn duration_ms(&self) -> u64 {
        self.entries.iter().map(|e| e.duration_ms).sum()
    }

    /// Where it plays from: the Sounds set for this playlist apply.
    pub fn source_context(&self) -> SourceContext {
        SourceContext::new(PLAYLIST_KIND, self.id.clone(), self.name.clone())
    }

    /// "Search again": keep `old`'s id, creation time and TIDAL copy.
    pub fn keep_identity_of(&mut self, old: &HomePlaylist) {
        self.id = old.id.clone();
        self.created_ms = old.created_ms;
        self.tidal_uuid = old.tidal_uuid.clone();
    }
}

/// A new playlist id: `p`, the time in ms and a 12-bit counter, in hex.
pub fn new_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    format!("p{:x}{:x}", now_ms(), COUNTER.fetch_add(1, Ordering::Relaxed) & ID_COUNTER_MASK)
}

/// Unix ms now (0 if the clock is before 1970).
pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The example file of the contract.
    const EXAMPLE: &str = r#"{
      "version": 1, "id": "p19a3c2f1e7a2", "name": "Korn · Tour setlist", "description": "From 8 recent shows",
      "created_ms": 1793640000000, "updated_ms": 1793640000000,
      "origin": {
        "kind": "concert", "artist": "Korn",
        "source": { "kind": "ai", "provider_id": "anthropic", "model": "claude-sonnet-5-5" },
        "mode": { "kind": "tour" }, "shows_wanted": 8,
        "thresholds": { "always_pct": 80, "usually_pct": 50, "sometimes_pct": 25, "encore_pct": 50 },
        "shows": [
          { "id": "2026-10-24-0", "date": "2026-10-24", "venue": "Arena CDMX", "city": "Mexico City",
            "country": "MX", "tour": "2026 Tour", "url": "https://www.example.com/korn-2026-10-24",
            "songs": [ { "name": "Intro", "original_artist": null, "tape": true, "encore": false },
                       { "name": "Blind", "original_artist": null, "tape": false, "encore": false } ] }
        ],
        "fetched_ms": 1793640000000
      },
      "entries": [
        { "title": "Blind", "artist": "Korn", "album": "Korn", "tidal_id": "1234567", "isrc": "USSM19400123",
          "local_path": null, "duration_ms": 259000, "explicit": true,
          "cover_url": "https://resources.tidal.com/images/x/320x320.jpg",
          "tag": "always", "note": "Played in 8 of 8 shows" }
      ],
      "unmatched": [ { "title": "Some Jam", "artist": "Korn", "tag": "sometimes", "note": "Played in 2 of 8 shows" } ],
      "sources": [ { "url": "https://www.example.com/korn-2026-10-24", "title": "Korn live in Mexico City" } ],
      "search_widget_html": null, "tidal_uuid": "0b1c2d3e-aaaa-bbbb-cccc-1234567890ab"
    }"#;

    #[test]
    fn the_contract_example_round_trips() {
        let playlist: HomePlaylist = serde_json::from_str(EXAMPLE).unwrap();
        assert_eq!(playlist.entries[0].tag, EntryTag::Always);
        assert_eq!(playlist.duration_ms(), 259_000);
        let PlaylistOrigin::Concert(origin) = &playlist.origin else { panic!("a concert") };
        assert_eq!(
            origin.source,
            SetlistSource::Ai { provider_id: "anthropic".into(), model: "claude-sonnet-5-5".into() }
        );
        assert_eq!(origin.shows[0].songs.len(), 2);

        let value = serde_json::to_value(&playlist).unwrap();
        let expected: serde_json::Value = serde_json::from_str(EXAMPLE).unwrap();
        assert_eq!(value, expected);
    }

    #[test]
    fn unknown_tags_read_as_plain() {
        let tag: EntryTag = serde_json::from_str(r#""must_have""#).unwrap();
        assert_eq!(tag, EntryTag::Plain);
        assert_eq!(serde_json::to_string(&EntryTag::Encore).unwrap(), r#""encore""#);
    }

    #[test]
    fn old_files_without_new_fields_still_parse() {
        let json = r#"{"id":"p1","name":"X","origin":{"kind":"concert","artist":"A"},"future":true}"#;
        let playlist: HomePlaylist = serde_json::from_str(json).unwrap();
        assert_eq!(playlist.version, PLAYLIST_FORMAT_VERSION);
        assert!(playlist.sources.is_empty() && playlist.search_widget_html.is_none());
        assert!(playlist.tidal_uuid.is_none(), "not saved to TIDAL");
    }

    #[test]
    fn a_party_playlist_round_trips() {
        let json = serde_json::json!({
            "version": 1, "id": "p1", "name": "Dance party · 2 h", "description": "For friends",
            "created_ms": 1, "updated_ms": 1,
            "origin": {
                "kind": "party",
                "brief": { "guests": "friends", "ages": ["young_adults", "adults"], "vibe": "dance", "minutes": 120,
                           "curve": "arc", "allow_explicit": true, "notes": "" },
                "provider_id": "anthropic", "model": "claude-sonnet-5-5", "built_ms": 1793640000000u64
            },
            "entries": [], "unmatched": [], "sources": [], "search_widget_html": null, "tidal_uuid": null
        });
        let playlist: HomePlaylist = serde_json::from_value(json.clone()).unwrap();
        let PlaylistOrigin::Party(origin) = &playlist.origin else { panic!("a party") };
        assert_eq!(origin.brief.ages.len(), 2);
        assert_eq!(origin.model, "claude-sonnet-5-5");
        assert_eq!(serde_json::to_value(&playlist).unwrap(), json);
    }

    #[test]
    fn a_chat_playlist_round_trips() {
        let json = serde_json::json!({
            "version": 1, "id": "p2", "name": "Sad 90s in English · 1 h", "description": "sad 90s in english",
            "created_ms": 1, "updated_ms": 1,
            "origin": {
                "kind": "chat",
                "brief": { "wish": "sad 90s in english", "title": "Sad 90s in English", "minutes": 60,
                           "curve": "steady", "allow_explicit": true },
                "provider_id": "anthropic", "model": "claude-sonnet-5-5", "built_ms": 1793640000000u64
            },
            "entries": [], "unmatched": [], "sources": [], "search_widget_html": null, "tidal_uuid": null
        });
        let playlist: HomePlaylist = serde_json::from_value(json.clone()).unwrap();
        let PlaylistOrigin::Chat(origin) = &playlist.origin else { panic!("a chat playlist") };
        assert_eq!(origin.brief.title, "Sad 90s in English");
        assert_eq!(serde_json::to_value(&playlist).unwrap(), json);
    }

    #[test]
    fn a_ride_playlist_round_trips_and_old_files_are_unaffected() {
        let json = serde_json::json!({
            "version": 1, "id": "p3", "name": "Ride", "description": "",
            "created_ms": 1, "updated_ms": 1,
            "origin": { "kind": "ride", "workout_id": "w1",
                        "wish": { "genres": ["rock", "metal"], "mix": true, "sentence": "90s" } },
            "entries": [], "unmatched": [], "sources": [], "search_widget_html": null, "tidal_uuid": null
        });
        let playlist: HomePlaylist = serde_json::from_value(json.clone()).unwrap();
        let PlaylistOrigin::Ride(origin) = &playlist.origin else { panic!("a ride") };
        assert_eq!(origin.wish.genres, ["rock", "metal"]);
        assert_eq!(serde_json::to_value(&playlist).unwrap(), json);
        // a ride written by a future version with fewer fields still reads
        let lean: PlaylistOrigin = serde_json::from_str(r#"{"kind":"ride"}"#).unwrap();
        assert_eq!(lean, PlaylistOrigin::Ride(RideOrigin::default()));
    }

    #[test]
    fn identity_and_source() {
        let old = HomePlaylist {
            id: "p1".into(),
            name: "Old".into(),
            created_ms: 7,
            tidal_uuid: Some("u1".into()),
            ..HomePlaylist::default()
        };
        let mut fresh = HomePlaylist { id: "p2".into(), name: "New".into(), created_ms: 9, ..HomePlaylist::default() };
        fresh.keep_identity_of(&old);
        assert_eq!((fresh.id.as_str(), fresh.created_ms), ("p1", 7));
        assert_eq!(fresh.tidal_uuid.as_deref(), Some("u1"), "a rebuild updates the same TIDAL copy");
        let ctx = fresh.source_context();
        assert!(ctx.is("playlist") && ctx.id == "p1" && ctx.name == "New");
    }

    #[test]
    fn ids_are_unique_and_safe() {
        let a = new_id();
        let b = new_id();
        assert_ne!(a, b);
        assert!(a.starts_with('p') && a.len() <= 64);
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}
