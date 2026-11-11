//! Playlist options, the same JSON on desktop (`desktop.json`) and Android.

use serde::{Deserialize, Deserializer, Serialize};

use crate::concert::{ConcertThresholds, DEFAULT_CONCERT_SHOWS, MAX_CONCERT_SHOWS, MIN_CONCERT_SHOWS};
use crate::party::{
    round_minutes, EnergyCurve, DEFAULT_CHAT_MINUTES, DEFAULT_OPEN_CHATS, DEFAULT_PARTY_MINUTES, DEFAULT_PER_ARTIST,
    MAX_OPEN_CHATS, MAX_PER_ARTIST, MIN_OPEN_CHATS, MIN_PER_ARTIST,
};

pub const MIN_SEARCH_BUDGET: u8 = 1;
pub const MAX_SEARCH_BUDGET: u8 = 10;
pub const DEFAULT_SEARCH_BUDGET: u8 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaylistSettings {
    /// Recent shows the AI looks up and the tour set considers, 3..=15.
    pub concert_shows: u8,
    pub concert_thresholds: ConcertThresholds,
    /// Local copies play instead of streaming (entries keep `local_path`).
    pub prefer_library: bool,
    /// Web searches the AI may run per build, 1..=10.
    pub search_budget: u8,
    /// Recorded intros and outros are left out.
    pub skip_tape: bool,
    /// Covers are searched under the original artist.
    pub covers_original_artist: bool,
    /// The result shows the list of cited pages (Google's widget is shown whenever present).
    pub show_sources: bool,
    /// "Made by HomeFi" section on the Playlists screen.
    pub home_section: bool,
    /// Default length of a party playlist (min), 30..=480 in steps of 15.
    pub party_minutes: u16,
    pub party_curve: EnergyCurve,
    pub party_allow_explicit: bool,
    /// Most songs of one artist in an AI-made playlist (party and chat), 1..=10.
    pub party_max_per_artist: u8,
    /// Default length of a chat ("Playlist with AI") playlist (min), 30..=480 in steps of 15.
    pub chat_minutes: u16,
    pub chat_curve: EnergyCurve,
    pub chat_allow_explicit: bool,
    /// "Playlist like this" in song menus and the player (opens the chat from a song).
    pub chat_from_song: bool,
    /// Most AI playlist chats open at once (party and chat), 1..=10.
    pub chat_max_open: u8,
    /// A notice when a chat's playlist finishes (or fails) while another screen is open.
    pub chat_done_notice: bool,
    /// A playlist HomeFi saves is also saved to the TIDAL account (new ones are created there,
    /// edits of one already there update it).
    pub tidal_auto_save: bool,
    /// Deleting a HomeFi playlist also deletes its copy in the TIDAL account.
    pub tidal_delete_with: bool,
}

impl Default for PlaylistSettings {
    fn default() -> Self {
        Self {
            concert_shows: DEFAULT_CONCERT_SHOWS,
            concert_thresholds: ConcertThresholds::default(),
            prefer_library: true,
            search_budget: DEFAULT_SEARCH_BUDGET,
            skip_tape: true,
            covers_original_artist: true,
            show_sources: true,
            home_section: true,
            party_minutes: DEFAULT_PARTY_MINUTES,
            party_curve: EnergyCurve::default(),
            party_allow_explicit: true,
            party_max_per_artist: DEFAULT_PER_ARTIST,
            chat_minutes: DEFAULT_CHAT_MINUTES,
            chat_curve: EnergyCurve::Steady,
            chat_allow_explicit: true,
            chat_from_song: true,
            chat_max_open: DEFAULT_OPEN_CHATS,
            chat_done_notice: true,
            tidal_auto_save: false,
            tidal_delete_with: false,
        }
    }
}

impl PlaylistSettings {
    pub fn clamped(self) -> Self {
        Self {
            concert_shows: self.concert_shows.clamp(MIN_CONCERT_SHOWS, MAX_CONCERT_SHOWS),
            concert_thresholds: self.concert_thresholds.clamped(),
            search_budget: self.search_budget.clamp(MIN_SEARCH_BUDGET, MAX_SEARCH_BUDGET),
            party_minutes: round_minutes(self.party_minutes),
            chat_minutes: round_minutes(self.chat_minutes),
            party_max_per_artist: self.party_max_per_artist.clamp(MIN_PER_ARTIST, MAX_PER_ARTIST),
            chat_max_open: self.chat_max_open.clamp(MIN_OPEN_CHATS, MAX_OPEN_CHATS),
            ..self
        }
    }
}

/// [`PlaylistSettings`] falling back to the defaults when unreadable.
pub fn lenient_playlist_settings<'de, D: Deserializer<'de>>(d: D) -> Result<PlaylistSettings, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Any {
        Known(PlaylistSettings),
        Other(serde::de::IgnoredAny),
    }
    Ok(match Any::deserialize(d)? {
        Any::Known(s) => s.clamped(),
        Any::Other(_) => PlaylistSettings::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_contract() {
        let json = serde_json::to_value(PlaylistSettings::default()).unwrap();
        let expected = serde_json::json!({
            "concert_shows": 8,
            "concert_thresholds": { "always_pct": 80, "usually_pct": 50, "sometimes_pct": 25, "encore_pct": 50 },
            "prefer_library": true, "search_budget": 5, "skip_tape": true,
            "covers_original_artist": true, "show_sources": true, "home_section": true,
            "party_minutes": 120, "party_curve": "arc", "party_allow_explicit": true, "party_max_per_artist": 3,
            "chat_minutes": 60, "chat_curve": "steady", "chat_allow_explicit": true, "chat_from_song": true,
            "chat_max_open": 3, "chat_done_notice": true,
            "tidal_auto_save": false, "tidal_delete_with": false
        });
        assert_eq!(json, expected);
    }

    #[test]
    fn clamps_out_of_range_values() {
        let s = PlaylistSettings { concert_shows: 99, search_budget: 0, ..PlaylistSettings::default() }.clamped();
        assert_eq!((s.concert_shows, s.search_budget), (MAX_CONCERT_SHOWS, MIN_SEARCH_BUDGET));
        let s = PlaylistSettings { concert_shows: 0, search_budget: 99, ..PlaylistSettings::default() }.clamped();
        assert_eq!((s.concert_shows, s.search_budget), (MIN_CONCERT_SHOWS, MAX_SEARCH_BUDGET));
    }

    #[test]
    fn party_settings_clamp_and_old_files_get_defaults() {
        let s =
            PlaylistSettings { party_minutes: 999, party_max_per_artist: 0, ..PlaylistSettings::default() }.clamped();
        assert_eq!((s.party_minutes, s.party_max_per_artist), (480, 1));
        let s =
            PlaylistSettings { party_minutes: 127, party_max_per_artist: 99, ..PlaylistSettings::default() }.clamped();
        assert_eq!((s.party_minutes, s.party_max_per_artist), (120, 10));
        // a file written by the concert-only version
        let old = r#"{"concert_shows":10,"prefer_library":false,"home_section":false}"#;
        let s: PlaylistSettings = serde_json::from_str(old).unwrap();
        assert_eq!((s.concert_shows, s.prefer_library), (10, false));
        assert_eq!(
            (s.party_minutes, s.party_curve, s.party_allow_explicit, s.party_max_per_artist),
            (120, EnergyCurve::Arc, true, 3)
        );
    }

    #[test]
    fn chat_settings_clamp_and_old_files_get_defaults() {
        let s = PlaylistSettings { chat_minutes: 999, ..PlaylistSettings::default() }.clamped();
        assert_eq!(s.chat_minutes, 480);
        let s = PlaylistSettings { chat_minutes: 127, ..PlaylistSettings::default() }.clamped();
        assert_eq!(s.chat_minutes, 120);
        let s: PlaylistSettings = serde_json::from_str(r#"{"party_minutes":90}"#).unwrap();
        assert_eq!((s.chat_minutes, s.chat_curve, s.chat_allow_explicit), (60, EnergyCurve::Steady, true));
        assert!(s.chat_from_song, "old files get the song entry");
        let off: PlaylistSettings = serde_json::from_str(r#"{"chat_from_song":false}"#).unwrap();
        assert!(!off.chat_from_song);
        assert_eq!((s.chat_max_open, s.chat_done_notice), (3, true), "old files get the defaults");
        let s = PlaylistSettings { chat_max_open: 0, ..PlaylistSettings::default() }.clamped();
        assert_eq!(s.chat_max_open, 1);
        let s = PlaylistSettings { chat_max_open: 99, ..PlaylistSettings::default() }.clamped();
        assert_eq!(s.chat_max_open, 10);
    }

    #[test]
    fn tidal_settings_are_off_by_default_and_travel() {
        let s: PlaylistSettings = serde_json::from_str(r#"{"chat_from_song":false}"#).unwrap();
        assert!(!s.tidal_auto_save && !s.tidal_delete_with, "old files never write to TIDAL");
        let on: PlaylistSettings =
            serde_json::from_str(r#"{"tidal_auto_save":true,"tidal_delete_with":true}"#).unwrap();
        assert!(on.clamped().tidal_auto_save && on.clamped().tidal_delete_with);
    }

    #[test]
    fn partial_json_gets_defaults() {
        let s: PlaylistSettings =
            serde_json::from_str(r#"{"skip_tape":false,"concert_thresholds":{"always_pct":90}}"#).unwrap();
        assert!(!s.skip_tape && s.prefer_library);
        assert_eq!(s.concert_thresholds.always_pct, 90);
        assert_eq!(s.concert_thresholds.usually_pct, 50);
        let back: PlaylistSettings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn lenient_reading() {
        #[derive(Deserialize)]
        struct Holder {
            #[serde(default, deserialize_with = "lenient_playlist_settings")]
            playlists: PlaylistSettings,
        }
        let h: Holder = serde_json::from_str(r#"{"playlists": 5}"#).unwrap();
        assert_eq!(h.playlists, PlaylistSettings::default());
        let h: Holder = serde_json::from_str(r#"{"playlists": {"concert_shows": 200}}"#).unwrap();
        assert_eq!(h.playlists.concert_shows, MAX_CONCERT_SHOWS);
    }
}
