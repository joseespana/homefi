//! "Make a party playlist": a few tappable questions (who is coming, ages,
//! vibe, length, energy, explicit lyrics), an optional chat with the user's own
//! AI, and a playlist made of songs the AI proposed and HomeFi found on Tidal
//! or in the library. AI only: without one there is nothing to build.
//!
//! The same session also makes "Playlist with AI" playlists: no questions, the
//! brief is what the user says in the chat ([`ChatBrief`]). [`Brief`] is either.
//!
//! [`PartyBrief`] is the answers, [`PartySession`] the chat and the build, and
//! [`arrange`] the pure part (duration, per-artist cap, energy curve).

use serde::{Deserialize, Deserializer, Serialize};

use crate::humane::Lang;
use crate::playlist::{ChatOrigin, PartyOrigin, PlaylistOrigin, PlaylistSettings};

mod arrange;
mod chat;
mod session;
pub(crate) mod words;

pub use arrange::{arrange, cap_per_artist, pick_for_duration, Picked};
pub use chat::{song_wish, AskedSong, ChatBrief, SongSeed, MAX_ASKED_SONGS};
pub use session::{
    needs_ai_message, no_session_message, too_many_chats_message, PartyBuilt, PartyContext, PartyProgress,
    PartySession, PartyTurn,
};

pub const MIN_PARTY_MINUTES: u16 = 30;
pub const MAX_PARTY_MINUTES: u16 = 480;
pub const DEFAULT_PARTY_MINUTES: u16 = 120;
/// Lengths are rounded to this.
pub const PARTY_MINUTES_STEP: u16 = 15;
pub const MIN_PER_ARTIST: u8 = 1;
pub const MAX_PER_ARTIST: u8 = 10;
pub const DEFAULT_PER_ARTIST: u8 = 3;
/// AI playlist chats open at once (party and chat), the setting's range.
pub const MIN_OPEN_CHATS: u8 = 1;
pub const MAX_OPEN_CHATS: u8 = 10;
pub const DEFAULT_OPEN_CHATS: u8 = 3;
/// Longest "anything else" text kept (chars).
pub const MAX_NOTES_CHARS: usize = 500;
/// User turns in the optional chat.
pub const MAX_CHAT_TURNS: u8 = 6;

/// User turns of the free chat (the chat is the whole input, and the way to refine after a build).
pub const MAX_FREE_CHAT_TURNS: u8 = 12;
pub const DEFAULT_CHAT_MINUTES: u16 = 60;
/// Longest title the AI may give (chars).
pub const MAX_TITLE_CHARS: usize = 60;

const MS_PER_MINUTE: u64 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PartyGuests {
    Family,
    Coworkers,
    Mixed,
    #[default]
    #[serde(other)]
    Friends,
}

/// 0-12, 13-17, 18-29, 30-49, 50+.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgeGroup {
    Kids,
    Teens,
    YoungAdults,
    Adults,
    Seniors,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PartyVibe {
    Background,
    Chill,
    Singalong,
    Throwback,
    #[default]
    #[serde(other)]
    Dance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EnergyCurve {
    /// Keeps building to the end.
    Rising,
    /// The AI's order, no energy shaping.
    Steady,
    /// Warm up, peak around two thirds in, wind down.
    #[default]
    #[serde(other)]
    Arc,
}

impl EnergyCurve {
    /// The curve in plain words, for the prompts.
    pub(crate) fn prompt_phrase(self) -> &'static str {
        match self {
            Self::Arc => "warm up, peak around two thirds in, wind down",
            Self::Rising => "keep building to the end",
            Self::Steady => "steady energy",
        }
    }
}

/// What an AI-made playlist is for: the party answers, or what the user said in the chat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Brief {
    Party(PartyBrief),
    Chat(ChatBrief),
}

impl From<PartyBrief> for Brief {
    fn from(brief: PartyBrief) -> Self {
        Self::Party(brief)
    }
}

impl From<ChatBrief> for Brief {
    fn from(brief: ChatBrief) -> Self {
        Self::Chat(brief)
    }
}

impl Brief {
    pub fn clamped(self) -> Self {
        match self {
            Self::Party(b) => Self::Party(b.clamped()),
            Self::Chat(b) => Self::Chat(b.clamped()),
        }
    }

    pub fn minutes(&self) -> u16 {
        match self {
            Self::Party(b) => b.minutes,
            Self::Chat(b) => b.minutes,
        }
    }

    pub fn curve(&self) -> EnergyCurve {
        match self {
            Self::Party(b) => b.curve,
            Self::Chat(b) => b.curve,
        }
    }

    pub fn allow_explicit(&self) -> bool {
        match self {
            Self::Party(b) => b.allow_explicit,
            Self::Chat(b) => b.allow_explicit,
        }
    }

    /// The length asked for, in ms.
    pub fn target_ms(&self) -> u64 {
        match self {
            Self::Party(b) => b.target_ms(),
            Self::Chat(b) => b.target_ms(),
        }
    }

    /// User turns the chat allows before it is closed with fixed words.
    pub fn max_chat_turns(&self) -> u8 {
        match self {
            Self::Party(_) => MAX_CHAT_TURNS,
            Self::Chat(_) => MAX_FREE_CHAT_TURNS,
        }
    }

    pub fn as_chat(&self) -> Option<&ChatBrief> {
        match self {
            Self::Chat(b) => Some(b),
            Self::Party(_) => None,
        }
    }

    /// The brief as a bullet list, for the prompts.
    pub(crate) fn describe(&self) -> String {
        match self {
            Self::Party(b) => session::describe_brief(b),
            Self::Chat(b) => b.describe(),
        }
    }

    /// What is being made, for "Propose 12 real songs for this …".
    pub(crate) fn what(&self) -> &'static str {
        match self {
            Self::Party(_) => "party",
            Self::Chat(_) => "playlist",
        }
    }

    pub(crate) fn name(&self, lang: Lang) -> String {
        match self {
            Self::Party(b) => words::name(lang, b.vibe, b.minutes),
            Self::Chat(b) => words::chat_name(lang, &b.title, &b.wish, b.minutes),
        }
    }

    /// The conversation's title: [`Self::name`] without the length for chats.
    pub(crate) fn title(&self, lang: Lang) -> String {
        match self {
            Self::Party(b) => words::name(lang, b.vibe, b.minutes),
            Self::Chat(b) => words::chat_title(lang, &b.title, &b.wish),
        }
    }

    pub(crate) fn description(&self, lang: Lang) -> String {
        match self {
            Self::Party(b) => words::description(lang, b.guests, &b.ages),
            Self::Chat(b) => b.wish.clone(),
        }
    }

    pub(crate) fn origin(&self, provider_id: String, model: String, built_ms: u64) -> PlaylistOrigin {
        match self {
            Self::Party(brief) => {
                PlaylistOrigin::Party(PartyOrigin { brief: brief.clone(), provider_id, model, built_ms })
            }
            Self::Chat(brief) => {
                PlaylistOrigin::Chat(ChatOrigin { brief: brief.clone(), provider_id, model, built_ms })
            }
        }
    }
}

/// The answers to the party questions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PartyBrief {
    pub guests: PartyGuests,
    /// Empty = any age. Unknown values are dropped when reading; deduplicated by [`clamped`](Self::clamped).
    #[serde(deserialize_with = "lenient_ages")]
    pub ages: Vec<AgeGroup>,
    pub vibe: PartyVibe,
    /// 30..=480, rounded to 15.
    pub minutes: u16,
    pub curve: EnergyCurve,
    pub allow_explicit: bool,
    /// Free text, trimmed, at most [`MAX_NOTES_CHARS`] chars.
    pub notes: String,
}

impl Default for PartyBrief {
    fn default() -> Self {
        Self::from_settings(&PlaylistSettings::default())
    }
}

impl PartyBrief {
    /// The questions' defaults: length, energy and explicit lyrics from the
    /// settings; friends, any age, a dance party, no notes.
    pub fn from_settings(settings: &PlaylistSettings) -> Self {
        Self {
            guests: PartyGuests::default(),
            ages: Vec::new(),
            vibe: PartyVibe::default(),
            minutes: settings.party_minutes,
            curve: settings.party_curve,
            allow_explicit: settings.party_allow_explicit,
            notes: String::new(),
        }
        .clamped()
    }

    /// Every answer inside its range: length rounded and clamped, ages without
    /// repeats (first wins), notes trimmed and cut at [`MAX_NOTES_CHARS`].
    pub fn clamped(self) -> Self {
        let mut ages: Vec<AgeGroup> = Vec::with_capacity(self.ages.len());
        for age in self.ages {
            if !ages.contains(&age) {
                ages.push(age);
            }
        }
        Self {
            ages,
            minutes: round_minutes(self.minutes),
            notes: crate::ai::prompt::one_line(&self.notes, MAX_NOTES_CHARS),
            ..self
        }
    }

    /// The length asked for, in ms.
    pub fn target_ms(&self) -> u64 {
        u64::from(round_minutes(self.minutes)) * MS_PER_MINUTE
    }
}

/// To the nearest [`PARTY_MINUTES_STEP`], then into 30..=480.
pub(crate) fn round_minutes(minutes: u16) -> u16 {
    let step = u32::from(PARTY_MINUTES_STEP);
    let rounded = (u32::from(minutes) + step / 2) / step * step;
    let rounded = u16::try_from(rounded).unwrap_or(MAX_PARTY_MINUTES);
    rounded.clamp(MIN_PARTY_MINUTES, MAX_PARTY_MINUTES)
}

/// A list of ages where unknown values (or a value that isn't a list) are dropped, never an error.
fn lenient_ages<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<AgeGroup>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Item {
        Known(AgeGroup),
        Other(serde::de::IgnoredAny),
    }
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Any {
        List(Vec<Item>),
        Other(serde::de::IgnoredAny),
    }
    Ok(match Any::deserialize(d)? {
        Any::List(items) => items
            .into_iter()
            .filter_map(|i| match i {
                Item::Known(age) => Some(age),
                Item::Other(_) => None,
            })
            .collect(),
        Any::Other(_) => Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brief_with_minutes(minutes: u16) -> PartyBrief {
        PartyBrief { minutes, ..PartyBrief::default() }.clamped()
    }

    #[test]
    fn minutes_round_to_the_nearest_step_then_clamp() {
        let got: Vec<u16> =
            [0, 29, 999, 127, 128, 135, 480, u16::MAX].into_iter().map(|m| brief_with_minutes(m).minutes).collect();
        assert_eq!(got, [30, 30, 480, 120, 135, 135, 480, 480]);
        assert_eq!(brief_with_minutes(90).target_ms(), 90 * 60_000);
    }

    #[test]
    fn ages_are_lenient_and_deduplicated() {
        let b: PartyBrief = serde_json::from_str(r#"{"ages":["teens","martians","teens",5,"adults"]}"#).unwrap();
        assert_eq!(b.ages, [AgeGroup::Teens, AgeGroup::Teens, AgeGroup::Adults]);
        assert_eq!(b.clamped().ages, [AgeGroup::Teens, AgeGroup::Adults]);
        let b: PartyBrief = serde_json::from_str(r#"{"ages":["teens","martians","teens"]}"#).unwrap();
        assert_eq!(b.clamped().ages, [AgeGroup::Teens]);
        let b: PartyBrief = serde_json::from_str(r#"{"ages":"everyone"}"#).unwrap();
        assert!(b.ages.is_empty());
    }

    #[test]
    fn unknown_words_read_as_the_defaults() {
        let b: PartyBrief =
            serde_json::from_str(r#"{"guests":"aliens","vibe":"polka","curve":"zigzag","minutes":60}"#).unwrap();
        assert_eq!((b.guests, b.vibe, b.curve), (PartyGuests::Friends, PartyVibe::Dance, EnergyCurve::Arc));
        assert_eq!(b.minutes, 60);
        assert!(b.allow_explicit, "missing keys take the defaults");
    }

    #[test]
    fn notes_are_trimmed_and_cut_on_characters() {
        let long = format!("  {}  ", "ñ".repeat(MAX_NOTES_CHARS + 20));
        let b = PartyBrief { notes: long, ..PartyBrief::default() }.clamped();
        assert_eq!(b.notes.chars().count(), MAX_NOTES_CHARS);
        assert_eq!(PartyBrief { notes: "  hi \n".into(), ..PartyBrief::default() }.clamped().notes, "hi");
    }

    #[test]
    fn the_questions_start_from_the_settings() {
        let settings = PlaylistSettings {
            party_minutes: 60,
            party_curve: EnergyCurve::Rising,
            party_allow_explicit: false,
            ..PlaylistSettings::default()
        };
        let b = PartyBrief::from_settings(&settings);
        assert_eq!((b.minutes, b.curve, b.allow_explicit), (60, EnergyCurve::Rising, false));
        assert_eq!((b.guests, b.vibe), (PartyGuests::Friends, PartyVibe::Dance));
        assert!(b.ages.is_empty() && b.notes.is_empty());
        assert_eq!(PartyBrief::default().minutes, DEFAULT_PARTY_MINUTES);
    }

    #[test]
    fn the_contract_json() {
        let b = PartyBrief { ages: vec![AgeGroup::YoungAdults, AgeGroup::Adults], ..PartyBrief::default() };
        let expected = serde_json::json!({
            "guests": "friends", "ages": ["young_adults", "adults"], "vibe": "dance", "minutes": 120,
            "curve": "arc", "allow_explicit": true, "notes": ""
        });
        assert_eq!(serde_json::to_value(&b).unwrap(), expected);
    }
}
