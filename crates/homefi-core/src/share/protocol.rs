//! Wire protocol between the hub and its guests: JSON text frames over a
//! WebSocket, one message per frame, tagged by `type` (see
//! `docs/shared-control.md`).
//!
//! Everything is lenient on purpose: the hub and the guests may run different
//! app versions, so unknown fields are ignored, cosmetic fields default and
//! unknown message types decode to `Unknown` instead of failing.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::permissions::Permissions;

/// Where a queued song comes from. Serialized as one string, `tidal:<id>`,
/// `hub:<library id>` or `guest:<upload id>`; anything else is kept verbatim
/// so a newer guest's source survives a round trip through an older hub.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum Source {
    /// A Tidal track id, played with the hub's own account.
    Tidal(String),
    /// A track of the hub's local library (id chosen by the host app).
    Hub(String),
    /// Reserved: a file uploaded by a guest (later phase).
    Guest(String),
    Other(String),
}

impl From<String> for Source {
    fn from(s: String) -> Self {
        match s.split_once(':') {
            Some(("tidal", id)) => Source::Tidal(id.to_owned()),
            Some(("hub", id)) => Source::Hub(id.to_owned()),
            Some(("guest", id)) => Source::Guest(id.to_owned()),
            _ => Source::Other(s),
        }
    }
}

impl From<Source> for String {
    fn from(s: Source) -> Self {
        s.to_string()
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::Tidal(id) => write!(f, "tidal:{id}"),
            Source::Hub(id) => write!(f, "hub:{id}"),
            Source::Guest(id) => write!(f, "guest:{id}"),
            Source::Other(s) => f.write_str(s),
        }
    }
}

impl Default for Source {
    fn default() -> Self {
        Source::Other(String::new())
    }
}

/// A playable song as guests see it (search results, pages, `add`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShareTrack {
    pub source: Source,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: Option<u64>,
    /// Absolute `http(s)` URL (Tidal covers) or a path on the hub such as
    /// `/cover/<id>` (local library covers); see `GuestClient::resolve_url`.
    pub cover_url: Option<String>,
    pub explicit: bool,
}

/// A person in the session. The hub itself is one too (`guest_id` = `hub`),
/// so its songs carry a name and colour like everyone else's.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct Member {
    pub guest_id: String,
    pub name: String,
    /// `#RRGGBB`, chosen by the guest.
    pub color: String,
}

/// `guest_id` of the hub's own [`Member`].
pub const HUB_ID: &str = "hub";

impl Member {
    pub fn is_hub(&self) -> bool {
        self.guest_id == HUB_ID
    }
}

/// A song in the shared queue (or playing now).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct QueueItem {
    /// Unique within a hub session; never reused.
    pub queue_id: u64,
    #[serde(flatten)]
    pub track: ShareTrack,
    pub added_by: Member,
}

/// What a guest can browse into from a search result.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowseKind {
    Album,
    Artist,
    Playlist,
    #[default]
    #[serde(other)]
    Unknown,
}

/// An album, artist or playlist row (search results, page headers).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Collection {
    pub kind: BrowseKind,
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub cover_url: Option<String>,
}

/// Answer to `search`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchResults {
    /// The query this answers, so a guest typing fast can drop stale replies.
    pub q: String,
    pub tracks: Vec<ShareTrack>,
    pub albums: Vec<Collection>,
    pub artists: Vec<Collection>,
    pub playlists: Vec<Collection>,
}

/// Answer to `browse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Page {
    pub header: Collection,
    pub tracks: Vec<ShareTrack>,
}

/// The song playing now, served by the hub so guests can play along on their
/// own speakers (party mode).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioStream {
    /// The [`QueueItem::queue_id`] it belongs to.
    pub queue_id: u64,
    /// Path on the hub (`/audio/<queue_id>?k=<key>`); see `GuestClient::resolve_url`.
    /// Supports HTTP range requests, so players can start before it is all there.
    pub url: String,
    pub mime: String,
    pub size: u64,
}

/// Everything a guest shows: broadcast to everyone on every change.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub now_playing: Option<QueueItem>,
    pub position_ms: u64,
    /// Hub clock (milliseconds, see `GuestClient::hub_now_ms`) at which
    /// `position_ms` was true: guests add the time since then while playing.
    pub clock_ms: u64,
    pub playing: bool,
    /// Set while the hub streams its audio to the guests (party mode).
    pub audio: Option<AudioStream>,
    /// Party mode: the next and the previous song's files, so guests download them
    /// ahead and play them from their own storage (no gap, no cut when the air is busy).
    pub prefetch: Vec<AudioStream>,
    /// Upcoming songs, in play order (`now_playing` is not included).
    pub queue: Vec<QueueItem>,
    /// Connected guests (the hub is not listed).
    pub guests: Vec<Member>,
    /// Current rules, so guests can grey out what they may not do (they can
    /// change while connected).
    pub permissions: Permissions,
    /// `vote_skip` count for the current song and how many are needed.
    pub skip_votes: u32,
    pub skip_votes_needed: u32,
    /// Hub volume, 0.0..=1.0, when the host reports it.
    pub volume: Option<f32>,
}

/// Guest to hub.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GuestMessage {
    /// Must be the first message; anything else before it closes the socket.
    Hello {
        name: String,
        #[serde(default)]
        color: String,
        #[serde(default)]
        code: String,
        #[serde(default)]
        app_version: String,
    },
    Search {
        q: String,
    },
    Browse {
        kind: BrowseKind,
        id: String,
    },
    Add {
        item: ShareTrack,
        /// Play it before the guest's other songs (or first, in first-come order).
        #[serde(default)]
        next: bool,
    },
    Remove {
        queue_id: u64,
    },
    Move {
        queue_id: u64,
        /// Target index in `State::queue`.
        to: usize,
    },
    Skip,
    VoteSkip,
    Toggle,
    /// Play this queued song now (it moves to the front and the current one is skipped).
    /// Needs pause/volume control or "everyone can skip".
    Play {
        queue_id: u64,
    },
    Volume {
        /// 0.0..=1.0
        value: f32,
    },
    /// Clock sync: the hub answers right away with its own time.
    Clock {
        /// The guest's clock when it sent this (milliseconds).
        t0: u64,
    },
    /// Party mode: this guest has the song loaded and can start it at once (also sent
    /// by guests that don't play along), so the hub can start a new song for everyone together.
    Ready {
        queue_id: u64,
    },
    /// A message type this version does not know.
    #[serde(other)]
    Unknown,
}

/// Hub to guest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HubMessage {
    Welcome {
        guest_id: String,
        session_name: String,
        #[serde(default)]
        permissions: Permissions,
        #[serde(default)]
        state: State,
    },
    State(State),
    Results(SearchResults),
    Page(Page),
    /// Something to show briefly (e.g. "Waiting for José to approve").
    Notice {
        text: String,
    },
    /// The last request was refused. Before `welcome` it means the join failed
    /// and the hub closes the socket.
    Denied {
        reason: String,
    },
    /// Answer to a guest's `clock`.
    Clock {
        t0: u64,
        /// The hub's clock when it answered (milliseconds since the hub started).
        hub_ms: u64,
    },
    /// What the hub's music sounds like right now, ~15 times a second while playing, so
    /// guests without the audio (remote control) still see the visualizer and VU meters
    /// move. 0..=255: left / right level and the log-spaced spectrum bands. Short names:
    /// it is the most frequent message.
    Levels {
        l: u8,
        r: u8,
        b: Vec<u8>,
    },
    #[serde(other)]
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn source_round_trips_as_one_string() {
        for s in ["tidal:123", "hub:abc:def", "guest:u1", "spotify:x", ""] {
            let src = Source::from(s.to_owned());
            assert_eq!(serde_json::to_value(&src).unwrap(), json!(s));
        }
        assert_eq!(Source::from("hub:abc:def".to_owned()), Source::Hub("abc:def".into()));
        assert_eq!(Source::from("tidal:9".to_owned()), Source::Tidal("9".into()));
    }

    #[test]
    fn guest_messages_parse_from_realistic_json() {
        let hello: GuestMessage =
            serde_json::from_value(json!({"type": "hello", "name": "Ana", "color": "#ff8800", "code": "4821"}))
                .unwrap();
        assert!(
            matches!(hello, GuestMessage::Hello { ref name, ref app_version, .. } if name == "Ana" && app_version.is_empty())
        );

        let add: GuestMessage = serde_json::from_value(json!({
            "type": "add",
            "item": {"source": "tidal:251380837", "title": "Flowers", "artist": "Miley Cyrus",
                     "album": "Endless Summer Vacation", "duration_ms": 200000,
                     "cover_url": "https://resources.tidal.com/images/a/b/c/320x320.jpg", "extra": 1}
        }))
        .unwrap();
        let GuestMessage::Add { item, next } = add else { panic!("not add") };
        assert!(!next);
        assert_eq!(item.source, Source::Tidal("251380837".into()));
        assert!(!item.explicit);

        let skip: GuestMessage = serde_json::from_value(json!({"type": "vote_skip"})).unwrap();
        assert_eq!(skip, GuestMessage::VoteSkip);
        let future: GuestMessage = serde_json::from_value(json!({"type": "dance", "style": "macarena"})).unwrap();
        assert_eq!(future, GuestMessage::Unknown);
    }

    #[test]
    fn hub_messages_are_tagged_and_flat() {
        let item = QueueItem {
            queue_id: 7,
            track: ShareTrack { source: Source::Hub("42".into()), title: "Song".into(), ..Default::default() },
            added_by: Member { guest_id: "g1".into(), name: "Ana".into(), color: "#ff0000".into() },
        };
        let msg = HubMessage::State(State { queue: vec![item.clone()], ..Default::default() });
        let v = serde_json::to_value(&msg).unwrap();
        assert_eq!(v["type"], "state");
        assert_eq!(v["queue"][0]["source"], "hub:42");
        assert_eq!(v["queue"][0]["queue_id"], 7);
        assert_eq!(v["queue"][0]["added_by"]["name"], "Ana");
        let back: HubMessage = serde_json::from_value(v).unwrap();
        assert_eq!(back, msg);

        let denied: HubMessage = serde_json::from_str(r#"{"type":"denied","reason":"wrong join code"}"#).unwrap();
        assert_eq!(denied, HubMessage::Denied { reason: "wrong join code".into() });
        // A sparse state from an older hub still decodes.
        let sparse: HubMessage = serde_json::from_str(r#"{"type":"state","playing":true,"new_field":[1]}"#).unwrap();
        assert!(matches!(sparse, HubMessage::State(State { playing: true, .. })));
    }
}
