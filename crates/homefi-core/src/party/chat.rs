//! The free-chat brief of a "Playlist with AI": what the user asked for in
//! their own words, and what the AI understood of it through `set_playlist`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::session::PartyTurn;
use super::{round_minutes, words, EnergyCurve, MAX_NOTES_CHARS, MAX_TITLE_CHARS, MS_PER_MINUTE};
use crate::ai::prompt::{one_line, tagged};
use crate::ai::Tool;
use crate::humane::Lang;
use crate::playlist::PlaylistSettings;

pub(crate) const SET_PLAYLIST: &str = "set_playlist";
/// The `energy` words the AI may use; anything else is ignored (an unknown word reads as `Arc` elsewhere).
const ENERGY_WORDS: [&str; 3] = ["steady", "rising", "arc"];
/// Most songs the user can name for the playlist to include.
pub const MAX_ASKED_SONGS: usize = 10;
/// Longest title or artist of a named song (chars).
pub(super) const MAX_SONG_FIELD_CHARS: usize = 120;

/// A song the user named ("alors dance" → “Alors on danse” by Stromae, as the AI corrected it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AskedSong {
    pub title: String,
    pub artist: String,
}

impl AskedSong {
    /// Same song under another spelling or credit counts once.
    pub(crate) fn key(&self) -> (String, String) {
        (crate::playlist::artist_key(&self.artist), crate::text::song_key(&self.title))
    }
}

/// What the user asked for in the chat, and what the AI understood of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChatBrief {
    /// The user's first message, trimmed, at most [`MAX_NOTES_CHARS`] chars. The playlist's description.
    pub wish: String,
    /// The AI's short title ("Sad 90s in English"), at most [`MAX_TITLE_CHARS`]; empty until it sets one.
    pub title: String,
    /// 30..=480, rounded to 15.
    pub minutes: u16,
    pub curve: EnergyCurve,
    pub allow_explicit: bool,
    /// Songs the user named: they open the playlist, in this order, and the rest sounds like them.
    /// At most [`MAX_ASKED_SONGS`], no repeats, none without a title or an artist. Left out of the JSON when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub songs: Vec<AskedSong>,
}

impl Default for ChatBrief {
    fn default() -> Self {
        Self::from_settings(&PlaylistSettings::default())
    }
}

impl ChatBrief {
    /// Nothing asked yet; length, energy and explicit lyrics from the settings.
    pub fn from_settings(settings: &PlaylistSettings) -> Self {
        Self {
            wish: String::new(),
            title: String::new(),
            minutes: settings.chat_minutes,
            curve: settings.chat_curve,
            allow_explicit: settings.chat_allow_explicit,
            songs: Vec::new(),
        }
        .clamped()
    }

    /// Length rounded and clamped, wish and title trimmed and cut on characters, songs cleaned.
    pub fn clamped(self) -> Self {
        Self {
            wish: cut(&self.wish, MAX_NOTES_CHARS),
            title: cut(&self.title, MAX_TITLE_CHARS),
            minutes: round_minutes(self.minutes),
            songs: clean_songs(self.songs),
            ..self
        }
    }

    /// The user has said what they want (the first message), so there is something to build from.
    pub fn is_asked(&self) -> bool {
        !self.wish.is_empty()
    }

    /// The length asked for, in ms.
    pub fn target_ms(&self) -> u64 {
        u64::from(round_minutes(self.minutes)) * MS_PER_MINUTE
    }

    /// Merges one `set_playlist` call: unknown or malformed fields are ignored, present ones replace.
    pub(crate) fn apply(&mut self, arguments: &Value) {
        if let Some(title) = arguments.get("title").and_then(Value::as_str) {
            let title = cut(title, MAX_TITLE_CHARS);
            if !title.is_empty() {
                self.title = title;
            }
        }
        if let Some(minutes) = arguments.get("minutes").and_then(Value::as_f64).filter(|m| m.is_finite()) {
            // the cast saturates: 99999 reads as "as long as allowed"
            self.minutes = round_minutes(minutes.round().clamp(0.0, f64::from(u16::MAX)) as u16);
        }
        if let Some(word) = arguments.get("energy").and_then(Value::as_str) {
            let word = word.trim().to_lowercase();
            if ENERGY_WORDS.contains(&word.as_str()) {
                if let Ok(curve) = serde_json::from_value::<EnergyCurve>(Value::String(word)) {
                    self.curve = curve;
                }
            }
        }
        if let Some(allow) = arguments.get("allow_explicit").and_then(Value::as_bool) {
            self.allow_explicit = allow;
        }
        // the whole list every time: a song left out was dropped by the user
        if let Some(songs) = arguments.get("songs").and_then(Value::as_array) {
            let text = |song: &Value, key: &str| song.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
            let songs =
                songs.iter().map(|s| AskedSong { title: text(s, "title"), artist: text(s, "artist") }).collect();
            self.songs = clean_songs(songs);
        }
    }

    /// The brief as a bullet list, for the prompts.
    pub(crate) fn describe(&self) -> String {
        let title = if self.title.is_empty() { "not set yet" } else { self.title.as_str() };
        let wish = if self.wish.is_empty() { "nothing yet".to_owned() } else { tagged("user_words", &self.wish) };
        let explicit = if self.allow_explicit { "allowed" } else { "to avoid" };
        let mut text = format!(
            "- Asked for: {wish}\n- Title: {title}\n- Length: {}\n- Energy: {}\n- Explicit lyrics: {explicit}",
            words::length(self.minutes),
            self.curve.prompt_phrase()
        );
        if !self.songs.is_empty() {
            let songs: Vec<String> = self.songs.iter().map(|s| format!("{} – {}", s.artist, s.title)).collect();
            text.push_str(&format!(
                "\n- Songs to include (they open the playlist): {}",
                tagged("named_songs", &songs.join("; "))
            ));
        }
        text
    }
}

/// A song the user wants a playlist like ("Playlist like this song").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SongSeed {
    pub title: String,
    pub artist: String,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub year: Option<u16>,
    #[serde(default)]
    pub genre: Option<String>,
}

/// The first chat message built from `seed`, in `lang`: the wish asking for a playlist like the song, starting with it.
pub fn song_wish(seed: &SongSeed, lang: Lang) -> String {
    words::song_wish(seed, lang)
}

/// Trimmed and cut, without blanks or repeats (first wins), at most [`MAX_ASKED_SONGS`].
fn clean_songs(songs: Vec<AskedSong>) -> Vec<AskedSong> {
    let mut seen = std::collections::HashSet::new();
    songs
        .into_iter()
        .map(|s| AskedSong { title: cut(&s.title, MAX_SONG_FIELD_CHARS), artist: cut(&s.artist, MAX_SONG_FIELD_CHARS) })
        .filter(|s| !s.title.is_empty() && !s.artist.is_empty() && seen.insert(s.key()))
        .take(MAX_ASKED_SONGS)
        .collect()
}

/// On one line (line breaks and control characters become spaces) and cut at `max` characters.
fn cut(text: &str, max: usize) -> String {
    one_line(text, max)
}

pub(crate) fn set_playlist_tool() -> Tool {
    Tool {
        name: SET_PLAYLIST.into(),
        description: "Record what you learned about the playlist the user wants. Every field is optional.".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "title": { "type": "string", "description": "Short name of the playlist, at most 6 words, in the user's language" },
                "minutes": { "type": "integer", "minimum": super::MIN_PARTY_MINUTES, "maximum": super::MAX_PARTY_MINUTES },
                "allow_explicit": { "type": "boolean" },
                "songs": {
                    "type": "array", "maxItems": MAX_ASKED_SONGS,
                    "description": "Every song the user named for the playlist, the full list each time, with the official \
                        title and main artist even when the user misspelled it or gave no artist \
                        (\"alors dance\" is Alors on danse by Stromae)",
                    "items": { "type": "object", "properties": {
                        "title": { "type": "string" }, "artist": { "type": "string" }
                    }, "required": ["title", "artist"] }
                },
                "energy": {
                    "type": "string", "enum": ENERGY_WORDS,
                    "description": "steady: your order; rising: builds to the end; arc: warm up, peak, wind down"
                }
            }
        }),
    }
}

/// The fixed first turn (no AI call): the question and four example answers, in `lang`.
/// With a `wish` (keep chatting a saved playlist) the text recalls it.
pub(crate) fn opener(lang: Lang, wish: &str) -> PartyTurn {
    PartyTurn {
        text: words::chat_opener_text(lang, wish),
        quick_replies: words::chat_opener_replies(lang),
        chat_brief: None,
        title: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::DEFAULT_CHAT_MINUTES;
    use super::*;

    #[test]
    fn the_brief_clamps_and_cuts() {
        let at = |minutes| ChatBrief { minutes, ..ChatBrief::default() }.clamped().minutes;
        assert_eq!([0, 999, 127].map(at), [30, 480, 120]);
        let long = ChatBrief { wish: format!("  {}  ", "ñ".repeat(MAX_NOTES_CHARS + 9)), ..ChatBrief::default() };
        assert_eq!(long.clamped().wish.chars().count(), MAX_NOTES_CHARS);
        let titled = ChatBrief { title: format!(" {} ", "é".repeat(MAX_TITLE_CHARS + 5)), ..ChatBrief::default() };
        assert_eq!(titled.clamped().title.chars().count(), MAX_TITLE_CHARS);
        assert_eq!(ChatBrief { wish: " hi \n".into(), ..ChatBrief::default() }.clamped().wish, "hi");
        assert_eq!(ChatBrief::default().target_ms(), 60 * 60_000);
        assert_eq!(ChatBrief::default().minutes, DEFAULT_CHAT_MINUTES);
    }

    #[test]
    fn apply_is_lenient() {
        let mut b = ChatBrief { title: "Old".into(), ..ChatBrief::default() };
        b.apply(&json!({ "energy": "zigzag", "minutes": "two", "title": "  ", "allow_explicit": "yes" }));
        assert_eq!(b, ChatBrief { title: "Old".into(), ..ChatBrief::default() });
        b.apply(&json!({ "minutes": 95.0, "title": " Sad 90s ", "energy": "rising", "allow_explicit": false }));
        assert_eq!(
            (b.minutes, b.title.as_str(), b.curve, b.allow_explicit),
            (90, "Sad 90s", EnergyCurve::Rising, false)
        );
        b.apply(&json!({ "minutes": 99999 }));
        assert_eq!(b.minutes, 480);
        b.apply(&json!({ "minutes": -5 }));
        assert_eq!(b.minutes, 30);
        b.apply(&json!("not an object"));
        assert_eq!(b.title, "Sad 90s");
    }

    #[test]
    fn the_tool_is_named_for_the_model() {
        let tool = set_playlist_tool();
        assert_eq!(tool.name, "set_playlist");
        assert_eq!(tool.parameters["properties"]["energy"]["enum"], json!(["steady", "rising", "arc"]));
        assert!(tool.parameters.get("required").is_none(), "every field is optional");
    }

    #[test]
    fn the_contract_json() {
        let b = ChatBrief { wish: "sad 90s".into(), title: "Sad".into(), ..ChatBrief::default() };
        let expected =
            json!({ "wish": "sad 90s", "title": "Sad", "minutes": 60, "curve": "steady", "allow_explicit": true });
        assert_eq!(serde_json::to_value(&b).unwrap(), expected);
        let tagged = serde_json::to_value(super::super::Brief::Chat(b.clone())).unwrap();
        assert_eq!(tagged["kind"], "chat");
        assert_eq!(tagged["wish"], "sad 90s");
        let back: super::super::Brief = serde_json::from_value(tagged).unwrap();
        assert_eq!(back, super::super::Brief::Chat(b));
    }

    #[test]
    fn named_songs_are_cleaned_and_replaced_as_a_whole() {
        let mut b = ChatBrief::default();
        b.apply(&json!({ "songs": [
            { "title": " Alors on danse ", "artist": "Stromae" },
            { "title": "alors on danse", "artist": "stromae" },
            { "title": "", "artist": "Nobody" },
            { "title": "No artist" },
            { "title": "Papaoutai", "artist": "Stromae" }
        ] }));
        let names: Vec<(&str, &str)> = b.songs.iter().map(|s| (s.title.as_str(), s.artist.as_str())).collect();
        assert_eq!(names, [("Alors on danse", "Stromae"), ("Papaoutai", "Stromae")]);
        assert!(b
            .describe()
            .contains("Songs to include (they open the playlist): <named_songs>Stromae – Alors on danse; Stromae – Papaoutai</named_songs>"));
        b.apply(&json!({ "title": "x" }));
        assert_eq!(b.songs.len(), 2, "a call without songs keeps them");
        b.apply(&json!({ "songs": [] }));
        assert!(b.songs.is_empty(), "the full list each time: empty clears it");
        let many: Vec<Value> = (0..20).map(|n| json!({ "title": format!("S{n}"), "artist": "A" })).collect();
        b.apply(&json!({ "songs": many }));
        assert_eq!(b.songs.len(), MAX_ASKED_SONGS);
        let old: ChatBrief = serde_json::from_value(json!({ "wish": "w" })).unwrap();
        assert!(old.songs.is_empty() && old.is_asked(), "briefs saved before named songs still read");
        assert!(!ChatBrief::default().is_asked());
    }

    fn seed(title: &str, artist: &str) -> SongSeed {
        SongSeed { title: title.into(), artist: artist.into(), album: None, year: None, genre: None }
    }

    #[test]
    fn song_wish_in_both_languages() {
        let full = SongSeed { year: Some(1976), genre: Some("rock".into()), ..seed("Hotel California", "Eagles") };
        assert_eq!(
            song_wish(&full, Lang::En),
            "A playlist like “Hotel California” by Eagles (1976, rock): same style, era and mood. Start with it."
        );
        assert_eq!(
            song_wish(&full, Lang::Es),
            "Una playlist como “Hotel California” de Eagles (1976, rock): mismo estilo, época y ambiente. Empieza con ella."
        );
    }

    #[test]
    fn song_wish_omits_missing_parts() {
        let bare = seed(" Yesterday ", " The Beatles ");
        assert_eq!(
            song_wish(&bare, Lang::En),
            "A playlist like “Yesterday” by The Beatles: same style, era and mood. Start with it."
        );
        let year = SongSeed { year: Some(1965), genre: Some("  ".into()), ..bare.clone() };
        assert!(song_wish(&year, Lang::Es).contains("de The Beatles (1965):"));
        let genre = SongSeed { genre: Some("pop".into()), ..bare };
        assert!(song_wish(&genre, Lang::En).contains("Beatles (pop):"));
    }

    #[test]
    fn song_wish_caps_long_fields_and_fits_the_wish() {
        let long = seed(&"t".repeat(500), &"a".repeat(500));
        let wish = song_wish(&long, Lang::Es);
        assert!(wish.chars().count() <= MAX_NOTES_CHARS);
        assert!(wish.ends_with("Empieza con ella."));
        let brief = ChatBrief { wish, ..ChatBrief::default() }.clamped();
        assert!(brief.wish.ends_with("Empieza con ella."), "the clamp does not cut it");
    }

    #[test]
    fn seed_json_contract() {
        let s: SongSeed = serde_json::from_value(json!({ "title": "A", "artist": "B" })).unwrap();
        assert_eq!(s, seed("A", "B"));
        let s: SongSeed = serde_json::from_value(json!({ "title": "A", "artist": "B", "year": 1999 })).unwrap();
        assert_eq!(s.year, Some(1999));
    }

    #[test]
    fn openers_in_both_languages() {
        for lang in [Lang::En, Lang::Es] {
            let turn = opener(lang, "");
            assert_eq!(turn.quick_replies.len(), 4);
            assert!(turn.quick_replies.iter().all(|r| r.chars().count() <= 48));
            assert!(opener(lang, "sad 90s").text.contains("sad 90s"));
        }
        assert!(opener(Lang::En, "").text.starts_with("What kind of playlist do you want today?"));
        assert!(opener(Lang::Es, "").text.starts_with("¿Qué tipo de playlist quieres hoy?"));
    }
}
