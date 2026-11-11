//! HomeFi's own playlists and "Get ready for a concert" for the Android app.
//!
//! Records and enums mirror `homefi_core::{playlist, concert, party}` one to one and
//! are mapped explicitly both ways, so a change in the core never changes the
//! Kotlin API by accident. The core decides everything (the source chain, the
//! tags, the matching, the words); this file only converts and forwards.
//!
//! The AI key travels apart from the config, like in [`crate::ai`], and is
//! never part of a record.

use std::sync::Arc;

use homefi_core::concert as core_concert;
use homefi_core::humane::Lang;
use homefi_core::party as core_party;
use homefi_core::playlist as pl;

use crate::ai::{core_config, AiConfig};
use crate::{HomefiEngine, Result};

// ------------------------------------------------------------------ records

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum EntryTag {
    Always,
    Usually,
    Sometimes,
    Encore,
    Plain,
}

impl From<pl::EntryTag> for EntryTag {
    fn from(t: pl::EntryTag) -> Self {
        match t {
            pl::EntryTag::Always => Self::Always,
            pl::EntryTag::Usually => Self::Usually,
            pl::EntryTag::Sometimes => Self::Sometimes,
            pl::EntryTag::Encore => Self::Encore,
            pl::EntryTag::Plain => Self::Plain,
        }
    }
}

impl From<EntryTag> for pl::EntryTag {
    fn from(t: EntryTag) -> Self {
        match t {
            EntryTag::Always => Self::Always,
            EntryTag::Usually => Self::Usually,
            EntryTag::Sometimes => Self::Sometimes,
            EntryTag::Encore => Self::Encore,
            EntryTag::Plain => Self::Plain,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PlaylistEntry {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub tidal_id: Option<String>,
    pub isrc: Option<String>,
    pub local_path: Option<String>,
    pub duration_ms: u64,
    pub explicit: bool,
    pub cover_url: Option<String>,
    pub tag: EntryTag,
    pub note: String,
}

impl From<pl::PlaylistEntry> for PlaylistEntry {
    fn from(e: pl::PlaylistEntry) -> Self {
        Self {
            title: e.title,
            artist: e.artist,
            album: e.album,
            tidal_id: e.tidal_id,
            isrc: e.isrc,
            local_path: e.local_path,
            duration_ms: e.duration_ms,
            explicit: e.explicit,
            cover_url: e.cover_url,
            tag: e.tag.into(),
            note: e.note,
        }
    }
}

impl From<PlaylistEntry> for pl::PlaylistEntry {
    fn from(e: PlaylistEntry) -> Self {
        Self {
            title: e.title,
            artist: e.artist,
            album: e.album,
            tidal_id: e.tidal_id,
            isrc: e.isrc,
            local_path: e.local_path,
            duration_ms: e.duration_ms,
            explicit: e.explicit,
            cover_url: e.cover_url,
            tag: e.tag.into(),
            note: e.note,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SongCandidate {
    pub title: String,
    pub artist: String,
    pub tag: EntryTag,
    pub note: String,
}

impl From<pl::SongCandidate> for SongCandidate {
    fn from(c: pl::SongCandidate) -> Self {
        Self { title: c.title, artist: c.artist, tag: c.tag.into(), note: c.note }
    }
}

impl From<SongCandidate> for pl::SongCandidate {
    fn from(c: SongCandidate) -> Self {
        Self { title: c.title, artist: c.artist, tag: c.tag.into(), note: c.note }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SourceLink {
    pub url: String,
    pub title: String,
}

impl From<pl::SourceLink> for SourceLink {
    fn from(s: pl::SourceLink) -> Self {
        Self { url: s.url, title: s.title }
    }
}

impl From<SourceLink> for pl::SourceLink {
    fn from(s: SourceLink) -> Self {
        Self { url: s.url, title: s.title }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ConcertSong {
    pub name: String,
    pub original_artist: Option<String>,
    pub tape: bool,
    pub encore: bool,
}

impl From<core_concert::ConcertSong> for ConcertSong {
    fn from(s: core_concert::ConcertSong) -> Self {
        Self { name: s.name, original_artist: s.original_artist, tape: s.tape, encore: s.encore }
    }
}

impl From<ConcertSong> for core_concert::ConcertSong {
    fn from(s: ConcertSong) -> Self {
        Self { name: s.name, original_artist: s.original_artist, tape: s.tape, encore: s.encore }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ConcertShow {
    pub id: String,
    pub date: String,
    pub venue: String,
    pub city: String,
    pub country: String,
    pub tour: String,
    pub url: Option<String>,
    pub songs: Vec<ConcertSong>,
}

impl From<core_concert::ConcertShow> for ConcertShow {
    fn from(s: core_concert::ConcertShow) -> Self {
        Self {
            id: s.id,
            date: s.date,
            venue: s.venue,
            city: s.city,
            country: s.country,
            tour: s.tour,
            url: s.url,
            songs: s.songs.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ConcertShow> for core_concert::ConcertShow {
    fn from(s: ConcertShow) -> Self {
        Self {
            id: s.id,
            date: s.date,
            venue: s.venue,
            city: s.city,
            country: s.country,
            tour: s.tour,
            url: s.url,
            songs: s.songs.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum ConcertMode {
    Tour,
    Show { show_id: String },
}

impl From<core_concert::ConcertMode> for ConcertMode {
    fn from(m: core_concert::ConcertMode) -> Self {
        match m {
            core_concert::ConcertMode::Tour => Self::Tour,
            core_concert::ConcertMode::Show { show_id } => Self::Show { show_id },
        }
    }
}

impl From<ConcertMode> for core_concert::ConcertMode {
    fn from(m: ConcertMode) -> Self {
        match m {
            ConcertMode::Tour => Self::Tour,
            ConcertMode::Show { show_id } => Self::Show { show_id },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum SetlistSource {
    Ai { provider_id: String, model: String },
    TidalPopular,
}

impl From<pl::SetlistSource> for SetlistSource {
    fn from(s: pl::SetlistSource) -> Self {
        match s {
            pl::SetlistSource::Ai { provider_id, model } => Self::Ai { provider_id, model },
            pl::SetlistSource::TidalPopular => Self::TidalPopular,
        }
    }
}

impl From<SetlistSource> for pl::SetlistSource {
    fn from(s: SetlistSource) -> Self {
        match s {
            SetlistSource::Ai { provider_id, model } => Self::Ai { provider_id, model },
            SetlistSource::TidalPopular => Self::TidalPopular,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct ConcertThresholds {
    pub always_pct: u8,
    pub usually_pct: u8,
    pub sometimes_pct: u8,
    pub encore_pct: u8,
}

impl From<core_concert::ConcertThresholds> for ConcertThresholds {
    fn from(t: core_concert::ConcertThresholds) -> Self {
        Self {
            always_pct: t.always_pct,
            usually_pct: t.usually_pct,
            sometimes_pct: t.sometimes_pct,
            encore_pct: t.encore_pct,
        }
    }
}

impl From<ConcertThresholds> for core_concert::ConcertThresholds {
    fn from(t: ConcertThresholds) -> Self {
        Self {
            always_pct: t.always_pct,
            usually_pct: t.usually_pct,
            sometimes_pct: t.sometimes_pct,
            encore_pct: t.encore_pct,
        }
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ConcertOrigin {
    pub artist: String,
    pub source: SetlistSource,
    pub mode: ConcertMode,
    pub shows_wanted: u8,
    pub thresholds: ConcertThresholds,
    pub shows: Vec<ConcertShow>,
    pub fetched_ms: u64,
}

impl From<pl::ConcertOrigin> for ConcertOrigin {
    fn from(o: pl::ConcertOrigin) -> Self {
        Self {
            artist: o.artist,
            source: o.source.into(),
            mode: o.mode.into(),
            shows_wanted: o.shows_wanted,
            thresholds: o.thresholds.into(),
            shows: o.shows.into_iter().map(Into::into).collect(),
            fetched_ms: o.fetched_ms,
        }
    }
}

impl From<ConcertOrigin> for pl::ConcertOrigin {
    fn from(o: ConcertOrigin) -> Self {
        Self {
            artist: o.artist,
            source: o.source.into(),
            mode: o.mode.into(),
            shows_wanted: o.shows_wanted,
            thresholds: o.thresholds.into(),
            shows: o.shows.into_iter().map(Into::into).collect(),
            fetched_ms: o.fetched_ms,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PartyGuests {
    Friends,
    Family,
    Coworkers,
    Mixed,
}

impl From<core_party::PartyGuests> for PartyGuests {
    fn from(g: core_party::PartyGuests) -> Self {
        match g {
            core_party::PartyGuests::Friends => Self::Friends,
            core_party::PartyGuests::Family => Self::Family,
            core_party::PartyGuests::Coworkers => Self::Coworkers,
            core_party::PartyGuests::Mixed => Self::Mixed,
        }
    }
}

impl From<PartyGuests> for core_party::PartyGuests {
    fn from(g: PartyGuests) -> Self {
        match g {
            PartyGuests::Friends => Self::Friends,
            PartyGuests::Family => Self::Family,
            PartyGuests::Coworkers => Self::Coworkers,
            PartyGuests::Mixed => Self::Mixed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AgeGroup {
    Kids,
    Teens,
    YoungAdults,
    Adults,
    Seniors,
}

impl From<core_party::AgeGroup> for AgeGroup {
    fn from(a: core_party::AgeGroup) -> Self {
        match a {
            core_party::AgeGroup::Kids => Self::Kids,
            core_party::AgeGroup::Teens => Self::Teens,
            core_party::AgeGroup::YoungAdults => Self::YoungAdults,
            core_party::AgeGroup::Adults => Self::Adults,
            core_party::AgeGroup::Seniors => Self::Seniors,
        }
    }
}

impl From<AgeGroup> for core_party::AgeGroup {
    fn from(a: AgeGroup) -> Self {
        match a {
            AgeGroup::Kids => Self::Kids,
            AgeGroup::Teens => Self::Teens,
            AgeGroup::YoungAdults => Self::YoungAdults,
            AgeGroup::Adults => Self::Adults,
            AgeGroup::Seniors => Self::Seniors,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PartyVibe {
    Background,
    Chill,
    Dance,
    Singalong,
    Throwback,
}

impl From<core_party::PartyVibe> for PartyVibe {
    fn from(v: core_party::PartyVibe) -> Self {
        match v {
            core_party::PartyVibe::Background => Self::Background,
            core_party::PartyVibe::Chill => Self::Chill,
            core_party::PartyVibe::Dance => Self::Dance,
            core_party::PartyVibe::Singalong => Self::Singalong,
            core_party::PartyVibe::Throwback => Self::Throwback,
        }
    }
}

impl From<PartyVibe> for core_party::PartyVibe {
    fn from(v: PartyVibe) -> Self {
        match v {
            PartyVibe::Background => Self::Background,
            PartyVibe::Chill => Self::Chill,
            PartyVibe::Dance => Self::Dance,
            PartyVibe::Singalong => Self::Singalong,
            PartyVibe::Throwback => Self::Throwback,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum EnergyCurve {
    Arc,
    Rising,
    Steady,
}

impl From<core_party::EnergyCurve> for EnergyCurve {
    fn from(c: core_party::EnergyCurve) -> Self {
        match c {
            core_party::EnergyCurve::Arc => Self::Arc,
            core_party::EnergyCurve::Rising => Self::Rising,
            core_party::EnergyCurve::Steady => Self::Steady,
        }
    }
}

impl From<EnergyCurve> for core_party::EnergyCurve {
    fn from(c: EnergyCurve) -> Self {
        match c {
            EnergyCurve::Arc => Self::Arc,
            EnergyCurve::Rising => Self::Rising,
            EnergyCurve::Steady => Self::Steady,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PartyBrief {
    pub guests: PartyGuests,
    pub ages: Vec<AgeGroup>,
    pub vibe: PartyVibe,
    pub minutes: u16,
    pub curve: EnergyCurve,
    pub allow_explicit: bool,
    pub notes: String,
}

impl From<core_party::PartyBrief> for PartyBrief {
    fn from(b: core_party::PartyBrief) -> Self {
        Self {
            guests: b.guests.into(),
            ages: b.ages.into_iter().map(Into::into).collect(),
            vibe: b.vibe.into(),
            minutes: b.minutes,
            curve: b.curve.into(),
            allow_explicit: b.allow_explicit,
            notes: b.notes,
        }
    }
}

impl From<PartyBrief> for core_party::PartyBrief {
    fn from(b: PartyBrief) -> Self {
        Self {
            guests: b.guests.into(),
            ages: b.ages.into_iter().map(Into::into).collect(),
            vibe: b.vibe.into(),
            minutes: b.minutes,
            curve: b.curve.into(),
            allow_explicit: b.allow_explicit,
            notes: b.notes,
        }
    }
}

/// What the user asked for in the chat ("Playlist with AI") and what the AI understood of it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ChatBrief {
    pub wish: String,
    pub title: String,
    pub minutes: u16,
    pub curve: EnergyCurve,
    pub allow_explicit: bool,
    /// Songs the user named: they open the playlist, in this order.
    pub songs: Vec<AskedSong>,
}

/// A song the user named in the chat, as the AI corrected it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AskedSong {
    pub title: String,
    pub artist: String,
}

impl From<core_party::AskedSong> for AskedSong {
    fn from(s: core_party::AskedSong) -> Self {
        Self { title: s.title, artist: s.artist }
    }
}

impl From<AskedSong> for core_party::AskedSong {
    fn from(s: AskedSong) -> Self {
        Self { title: s.title, artist: s.artist }
    }
}

impl From<core_party::ChatBrief> for ChatBrief {
    fn from(b: core_party::ChatBrief) -> Self {
        Self {
            wish: b.wish,
            title: b.title,
            minutes: b.minutes,
            curve: b.curve.into(),
            allow_explicit: b.allow_explicit,
            songs: b.songs.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ChatBrief> for core_party::ChatBrief {
    fn from(b: ChatBrief) -> Self {
        Self {
            wish: b.wish,
            title: b.title,
            minutes: b.minutes,
            curve: b.curve.into(),
            allow_explicit: b.allow_explicit,
            songs: b.songs.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ChatOrigin {
    pub brief: ChatBrief,
    pub provider_id: String,
    pub model: String,
    pub built_ms: u64,
}

impl From<pl::ChatOrigin> for ChatOrigin {
    fn from(o: pl::ChatOrigin) -> Self {
        Self { brief: o.brief.into(), provider_id: o.provider_id, model: o.model, built_ms: o.built_ms }
    }
}

impl From<ChatOrigin> for pl::ChatOrigin {
    fn from(o: ChatOrigin) -> Self {
        Self { brief: o.brief.into(), provider_id: o.provider_id, model: o.model, built_ms: o.built_ms }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PartyOrigin {
    pub brief: PartyBrief,
    pub provider_id: String,
    pub model: String,
    pub built_ms: u64,
}

impl From<pl::PartyOrigin> for PartyOrigin {
    fn from(o: pl::PartyOrigin) -> Self {
        Self { brief: o.brief.into(), provider_id: o.provider_id, model: o.model, built_ms: o.built_ms }
    }
}

impl From<PartyOrigin> for pl::PartyOrigin {
    fn from(o: PartyOrigin) -> Self {
        Self { brief: o.brief.into(), provider_id: o.provider_id, model: o.model, built_ms: o.built_ms }
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum PlaylistOrigin {
    Concert { origin: ConcertOrigin },
    Party { origin: PartyOrigin },
    Chat { origin: ChatOrigin },
    Ride { origin: RideOrigin },
}

/// An Exercise ride's playlist: the workout and the music wish it was built from.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RideOrigin {
    pub workout_id: String,
    pub wish: crate::exercise::MusicWish,
}

impl From<pl::RideOrigin> for RideOrigin {
    fn from(o: pl::RideOrigin) -> Self {
        Self { workout_id: o.workout_id, wish: o.wish.into() }
    }
}

impl From<RideOrigin> for pl::RideOrigin {
    fn from(o: RideOrigin) -> Self {
        Self { workout_id: o.workout_id, wish: o.wish.into() }
    }
}

impl From<pl::PlaylistOrigin> for PlaylistOrigin {
    fn from(o: pl::PlaylistOrigin) -> Self {
        match o {
            pl::PlaylistOrigin::Concert(origin) => Self::Concert { origin: origin.into() },
            pl::PlaylistOrigin::Party(origin) => Self::Party { origin: origin.into() },
            pl::PlaylistOrigin::Chat(origin) => Self::Chat { origin: origin.into() },
            pl::PlaylistOrigin::Ride(origin) => Self::Ride { origin: origin.into() },
        }
    }
}

impl From<PlaylistOrigin> for pl::PlaylistOrigin {
    fn from(o: PlaylistOrigin) -> Self {
        match o {
            PlaylistOrigin::Concert { origin } => Self::Concert(origin.into()),
            PlaylistOrigin::Party { origin } => Self::Party(origin.into()),
            PlaylistOrigin::Chat { origin } => Self::Chat(origin.into()),
            PlaylistOrigin::Ride { origin } => Self::Ride(origin.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct HomePlaylist {
    pub version: u32,
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_ms: u64,
    pub updated_ms: u64,
    pub origin: PlaylistOrigin,
    pub entries: Vec<PlaylistEntry>,
    pub unmatched: Vec<SongCandidate>,
    pub sources: Vec<SourceLink>,
    pub search_widget_html: Option<String>,
    /// The copy in the user's TIDAL account; `None` until it's saved there.
    pub tidal_uuid: Option<String>,
}

impl From<pl::HomePlaylist> for HomePlaylist {
    fn from(p: pl::HomePlaylist) -> Self {
        Self {
            version: p.version,
            id: p.id,
            name: p.name,
            description: p.description,
            created_ms: p.created_ms,
            updated_ms: p.updated_ms,
            origin: p.origin.into(),
            entries: p.entries.into_iter().map(Into::into).collect(),
            unmatched: p.unmatched.into_iter().map(Into::into).collect(),
            sources: p.sources.into_iter().map(Into::into).collect(),
            search_widget_html: p.search_widget_html,
            tidal_uuid: p.tidal_uuid,
        }
    }
}

impl From<HomePlaylist> for pl::HomePlaylist {
    fn from(p: HomePlaylist) -> Self {
        Self {
            version: p.version,
            id: p.id,
            name: p.name,
            description: p.description,
            created_ms: p.created_ms,
            updated_ms: p.updated_ms,
            origin: p.origin.into(),
            entries: p.entries.into_iter().map(Into::into).collect(),
            unmatched: p.unmatched.into_iter().map(Into::into).collect(),
            sources: p.sources.into_iter().map(Into::into).collect(),
            search_widget_html: p.search_widget_html,
            tidal_uuid: p.tidal_uuid,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct PlaylistSettings {
    pub concert_shows: u8,
    pub concert_thresholds: ConcertThresholds,
    pub prefer_library: bool,
    pub search_budget: u8,
    pub skip_tape: bool,
    pub covers_original_artist: bool,
    pub show_sources: bool,
    pub home_section: bool,
    pub party_minutes: u16,
    pub party_curve: EnergyCurve,
    pub party_allow_explicit: bool,
    pub party_max_per_artist: u8,
    pub chat_minutes: u16,
    pub chat_curve: EnergyCurve,
    pub chat_allow_explicit: bool,
    pub chat_from_song: bool,
    pub chat_max_open: u8,
    pub chat_done_notice: bool,
    pub tidal_auto_save: bool,
    pub tidal_delete_with: bool,
}

impl From<pl::PlaylistSettings> for PlaylistSettings {
    fn from(s: pl::PlaylistSettings) -> Self {
        Self {
            concert_shows: s.concert_shows,
            concert_thresholds: s.concert_thresholds.into(),
            prefer_library: s.prefer_library,
            search_budget: s.search_budget,
            skip_tape: s.skip_tape,
            covers_original_artist: s.covers_original_artist,
            show_sources: s.show_sources,
            home_section: s.home_section,
            party_minutes: s.party_minutes,
            party_curve: s.party_curve.into(),
            party_allow_explicit: s.party_allow_explicit,
            party_max_per_artist: s.party_max_per_artist,
            chat_minutes: s.chat_minutes,
            chat_curve: s.chat_curve.into(),
            chat_allow_explicit: s.chat_allow_explicit,
            chat_from_song: s.chat_from_song,
            chat_max_open: s.chat_max_open,
            chat_done_notice: s.chat_done_notice,
            tidal_auto_save: s.tidal_auto_save,
            tidal_delete_with: s.tidal_delete_with,
        }
    }
}

impl From<PlaylistSettings> for pl::PlaylistSettings {
    fn from(s: PlaylistSettings) -> Self {
        Self {
            concert_shows: s.concert_shows,
            concert_thresholds: s.concert_thresholds.into(),
            prefer_library: s.prefer_library,
            search_budget: s.search_budget,
            skip_tape: s.skip_tape,
            covers_original_artist: s.covers_original_artist,
            show_sources: s.show_sources,
            home_section: s.home_section,
            party_minutes: s.party_minutes,
            party_curve: s.party_curve.into(),
            party_allow_explicit: s.party_allow_explicit,
            party_max_per_artist: s.party_max_per_artist,
            chat_minutes: s.chat_minutes,
            chat_curve: s.chat_curve.into(),
            chat_allow_explicit: s.chat_allow_explicit,
            chat_from_song: s.chat_from_song,
            chat_max_open: s.chat_max_open,
            chat_done_notice: s.chat_done_notice,
            tidal_auto_save: s.tidal_auto_save,
            tidal_delete_with: s.tidal_delete_with,
        }
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ConcertRequest {
    pub artist: String,
    pub shows: u8,
    pub thresholds: ConcertThresholds,
    pub mode: ConcertMode,
    pub popular_only: bool,
}

impl From<ConcertRequest> for core_concert::ConcertRequest {
    fn from(r: ConcertRequest) -> Self {
        Self {
            artist: r.artist,
            shows: r.shows,
            thresholds: r.thresholds.into(),
            mode: r.mode.into(),
            popular_only: r.popular_only,
        }
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ConcertBuilt {
    pub playlist: HomePlaylist,
    pub fallback_note: Option<String>,
}

impl From<core_concert::ConcertBuilt> for ConcertBuilt {
    fn from(b: core_concert::ConcertBuilt) -> Self {
        Self { playlist: b.playlist.into(), fallback_note: b.fallback_note }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ConcertProgress {
    Searching { with_ai: bool },
    ShowsFound { count: u32 },
    Resolving { done: u32, total: u32 },
}

/// Counts that don't fit a `u32` (they never do in practice) saturate.
pub(crate) fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl From<core_concert::ConcertProgress> for ConcertProgress {
    fn from(p: core_concert::ConcertProgress) -> Self {
        match p {
            core_concert::ConcertProgress::Searching { with_ai } => Self::Searching { with_ai },
            core_concert::ConcertProgress::ShowsFound { count: n } => Self::ShowsFound { count: count(n) },
            core_concert::ConcertProgress::Resolving { done, total } => {
                Self::Resolving { done: count(done), total: count(total) }
            }
        }
    }
}

/// A song file of the user's library, so playlist entries can play from it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LibraryRef {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub isrc: Option<String>,
    pub duration_ms: u64,
    /// The file's `BPM` tag (Exercise mode matches songs to a cadence with it).
    #[uniffi(default = None)]
    pub bpm: Option<u16>,
    /// The file's genre tag.
    #[uniffi(default = "")]
    pub genre: String,
}

impl From<LibraryRef> for pl::LibraryRef {
    fn from(l: LibraryRef) -> Self {
        Self {
            path: l.path,
            title: l.title,
            artist: l.artist,
            album: l.album,
            isrc: l.isrc,
            duration_ms: l.duration_ms,
            bpm: l.bpm,
            genre: l.genre,
        }
    }
}

/// What saving a playlist to TIDAL did.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TidalSaveReport {
    pub uuid: String,
    /// Made now (first save, or the old copy was deleted on TIDAL).
    pub created: bool,
    pub saved: u32,
    /// "Title · Artist" of the songs TIDAL doesn't have.
    pub skipped: Vec<String>,
    /// "Saved 24 songs to TIDAL · 2 aren't on TIDAL", in the language asked for.
    pub summary: String,
}

impl TidalSaveReport {
    fn new(report: pl::TidalSaveReport, lang: Lang) -> Self {
        let summary = report.summary(lang);
        Self {
            uuid: report.uuid,
            created: report.created,
            saved: count(report.saved),
            skipped: report.skipped,
            summary,
        }
    }
}

/// A saved playlist after it was saved to TIDAL: the stored copy (with its `tidal_uuid`) and the report.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct TidalSaved {
    pub playlist: HomePlaylist,
    pub report: TidalSaveReport,
}

// ------------------------------------------------------------------ engine

#[uniffi::export(callback_interface)]
pub trait ConcertListener: Send + Sync {
    fn on_progress(&self, progress: ConcertProgress);
}

/// Indexing thousands of files is CPU work: off the async workers.
pub(crate) async fn library_index(library: Vec<LibraryRef>) -> pl::LibraryIndex {
    tokio::task::spawn_blocking(move || pl::LibraryIndex::new(library.into_iter().map(Into::into).collect()))
        .await
        .unwrap_or_default()
}

#[uniffi::export]
impl HomefiEngine {
    /// Builds a concert playlist (not saved: call [`home_playlist_save`](Self::home_playlist_save)).
    /// The AI's key travels apart and is never stored. `replace_id`: the
    /// playlist this search replaces ("Search again"); it keeps its id and
    /// creation time. Cancelling the coroutine stops the work.
    #[allow(clippy::too_many_arguments)]
    pub async fn concert_build(
        self: Arc<Self>,
        request: ConcertRequest,
        settings: PlaylistSettings,
        ai: Option<AiConfig>,
        api_key: Option<String>,
        language: String,
        library: Vec<LibraryRef>,
        replace_id: Option<String>,
        listener: Box<dyn ConcertListener>,
    ) -> Result<ConcertBuilt> {
        crate::runtime::run(async move {
            // signed out is not an error: only the library and the AI are used
            let _ = self.session.refresh(600, false).await;
            let api = self.session.api().ok();
            let ai = ai.map(|c| core_config(c, api_key));
            let index = library_index(library).await;
            let settings = pl::PlaylistSettings::from(settings);
            let ctx = core_concert::BuildContext {
                settings: &settings,
                ai: ai.as_ref(),
                api: api.as_ref(),
                library: &index,
                lang: Lang::from_code(&language),
            };
            let mut built = core_concert::build(&request.into(), &ctx, &|p| listener.on_progress(p.into())).await?;
            if let Some(id) = replace_id {
                if let Ok(old) = pl::PlaylistStore::in_app_dir(self.session.app_dir()).load(&id) {
                    built.playlist.keep_identity_of(&old);
                }
            }
            Ok(built.into())
        })
        .await
    }

    /// Re-aggregates the shows stored in `playlist` (no AI call): another
    /// mode, fewer shows or other thresholds.
    pub async fn concert_rearrange(
        self: Arc<Self>,
        playlist: HomePlaylist,
        request: ConcertRequest,
        settings: PlaylistSettings,
        language: String,
        library: Vec<LibraryRef>,
        listener: Box<dyn ConcertListener>,
    ) -> Result<HomePlaylist> {
        crate::runtime::run(async move {
            let _ = self.session.refresh(600, false).await;
            let api = self.session.api().ok();
            let index = library_index(library).await;
            let settings = pl::PlaylistSettings::from(settings);
            let ctx = core_concert::BuildContext {
                settings: &settings,
                ai: None,
                api: api.as_ref(),
                library: &index,
                lang: Lang::from_code(&language),
            };
            let rebuilt =
                core_concert::rearrange(&playlist.into(), &request.into(), &ctx, &|p| listener.on_progress(p.into()))
                    .await?;
            Ok(rebuilt.into())
        })
        .await
    }

    /// The saved playlists, most recently updated first. Reads files: call it
    /// off the main thread.
    pub fn home_playlists(&self) -> Vec<HomePlaylist> {
        self.playlist_store().list().into_iter().map(Into::into).collect()
    }

    pub fn home_playlist_save(&self, playlist: HomePlaylist) -> Result<()> {
        Ok(self.playlist_store().save(&playlist.into())?)
    }

    /// Removes HomeFi's own copy; library files are never touched (its TIDAL copy is deleted
    /// apart, see [`home_playlist_delete_on_tidal`](Self::home_playlist_delete_on_tidal)).
    /// `false` = there was no such playlist.
    pub fn home_playlist_delete(&self, id: String) -> Result<bool> {
        Ok(self.playlist_store().remove(&id)?)
    }

    /// Deletes the copy of `playlist` in the user's TIDAL account (the "delete the TIDAL copy
    /// too" setting). `false` when it had none or it was already gone.
    pub async fn home_playlist_delete_on_tidal(self: Arc<Self>, playlist: HomePlaylist) -> Result<bool> {
        crate::runtime::run(async move {
            if playlist.tidal_uuid.is_none() {
                return Ok(false);
            }
            let _ = self.session.refresh(600, false).await;
            let api = self.session.api()?;
            Ok(pl::delete_from_tidal(&api, &playlist.into()).await?)
        })
        .await
    }

    /// Saves the stored playlist `id` to the user's TIDAL account: created there the first
    /// time, its items replaced after that so TIDAL matches HomeFi. Songs TIDAL doesn't have
    /// are skipped and listed. `language` words the summary ("en" or "es").
    pub async fn home_playlist_save_to_tidal(self: Arc<Self>, id: String, language: String) -> Result<TidalSaved> {
        crate::runtime::run(async move {
            let _ = self.session.refresh(600, false).await;
            let api = self.session.api()?;
            let (playlist, report) = pl::save_stored_to_tidal(&api, &self.playlist_store(), &id).await?;
            Ok(TidalSaved {
                playlist: playlist.into(),
                report: TidalSaveReport::new(report, Lang::from_code(&language)),
            })
        })
        .await
    }
}

impl HomefiEngine {
    fn playlist_store(&self) -> pl::PlaylistStore {
        pl::PlaylistStore::in_app_dir(self.session.app_dir())
    }
}

// ----------------------------------------------------------------- settings

#[uniffi::export]
pub fn playlist_default_settings() -> PlaylistSettings {
    pl::PlaylistSettings::default().into()
}

#[uniffi::export]
pub fn playlist_settings_clamped(settings: PlaylistSettings) -> PlaylistSettings {
    pl::PlaylistSettings::from(settings).clamped().into()
}

#[uniffi::export]
pub fn playlist_settings_to_json(settings: PlaylistSettings) -> String {
    serde_json::to_string(&pl::PlaylistSettings::from(settings)).unwrap_or_else(|_| "{}".into())
}

/// Unreadable JSON gives the defaults; readable JSON is clamped.
#[uniffi::export]
pub fn playlist_settings_from_json(json: String) -> PlaylistSettings {
    serde_json::from_str::<pl::PlaylistSettings>(&json).unwrap_or_default().clamped().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_playlist() -> pl::HomePlaylist {
        let show = core_concert::ConcertShow {
            id: "2026-10-24-0".into(),
            date: "2026-10-24".into(),
            venue: "Arena".into(),
            city: "Mexico City".into(),
            country: "MX".into(),
            tour: "2026 Tour".into(),
            url: Some("https://www.example.com/x".into()),
            songs: vec![
                core_concert::ConcertSong { name: "Blind".into(), ..Default::default() },
                core_concert::ConcertSong {
                    name: "Wicked Game".into(),
                    original_artist: Some("Chris Isaak".into()),
                    tape: false,
                    encore: true,
                },
            ],
        };
        pl::HomePlaylist {
            id: "p1".into(),
            name: "Korn".into(),
            description: "d".into(),
            created_ms: 1,
            updated_ms: 2,
            origin: pl::PlaylistOrigin::Concert(pl::ConcertOrigin {
                artist: "Korn".into(),
                source: pl::SetlistSource::Ai { provider_id: "anthropic".into(), model: "m".into() },
                mode: core_concert::ConcertMode::Show { show_id: show.id.clone() },
                shows_wanted: 8,
                thresholds: core_concert::ConcertThresholds::default(),
                shows: vec![show],
                fetched_ms: 3,
            }),
            entries: vec![pl::PlaylistEntry {
                title: "Blind".into(),
                artist: "Korn".into(),
                album: "Korn".into(),
                tidal_id: Some("9".into()),
                isrc: Some("US1".into()),
                local_path: Some("/m/blind.flac".into()),
                duration_ms: 259_000,
                explicit: true,
                cover_url: Some("https://example.com/c.jpg".into()),
                tag: pl::EntryTag::Always,
                note: "Played in 8 of 8 shows".into(),
            }],
            unmatched: vec![pl::SongCandidate {
                title: "Some Jam".into(),
                artist: "Korn".into(),
                tag: pl::EntryTag::Sometimes,
                note: "n".into(),
            }],
            sources: vec![pl::SourceLink { url: "https://www.example.com/x".into(), title: "t".into() }],
            search_widget_html: Some("<div></div>".into()),
            tidal_uuid: Some("0b1c2d3e".into()),
            ..pl::HomePlaylist::default()
        }
    }

    #[test]
    fn a_playlist_survives_the_round_trip() {
        let original = full_playlist();
        let mirrored = HomePlaylist::from(original.clone());
        assert_eq!(mirrored.tidal_uuid.as_deref(), Some("0b1c2d3e"));
        let back = pl::HomePlaylist::from(mirrored);
        assert_eq!(back, original);
        let tour = pl::PlaylistOrigin::Concert(pl::ConcertOrigin::default());
        assert_eq!(pl::PlaylistOrigin::from(PlaylistOrigin::from(tour.clone())), tour);
        let popular = pl::SetlistSource::TidalPopular;
        assert_eq!(pl::SetlistSource::from(SetlistSource::from(popular.clone())), popular);
    }

    fn party_playlist() -> pl::HomePlaylist {
        pl::HomePlaylist {
            id: "p2".into(),
            name: "Dance party · 2 h".into(),
            origin: pl::PlaylistOrigin::Party(pl::PartyOrigin {
                brief: core_party::PartyBrief {
                    guests: core_party::PartyGuests::Coworkers,
                    ages: vec![core_party::AgeGroup::YoungAdults, core_party::AgeGroup::Seniors],
                    vibe: core_party::PartyVibe::Throwback,
                    minutes: 135,
                    curve: core_party::EnergyCurve::Rising,
                    allow_explicit: false,
                    notes: "no reggaeton".into(),
                },
                provider_id: "anthropic".into(),
                model: "m".into(),
                built_ms: 5,
            }),
            ..pl::HomePlaylist::default()
        }
    }

    #[test]
    fn a_party_playlist_survives_the_round_trip() {
        let original = party_playlist();
        let mirrored = HomePlaylist::from(original.clone());
        let PlaylistOrigin::Party { origin } = &mirrored.origin else { panic!("a party origin") };
        assert_eq!(origin.brief.ages, [AgeGroup::YoungAdults, AgeGroup::Seniors]);
        assert_eq!(pl::HomePlaylist::from(mirrored), original);
    }

    #[test]
    fn a_chat_playlist_survives_the_round_trip() {
        let original = pl::HomePlaylist {
            id: "p3".into(),
            origin: pl::PlaylistOrigin::Chat(pl::ChatOrigin {
                brief: core_party::ChatBrief {
                    wish: "sad 90s".into(),
                    title: "Sad 90s".into(),
                    minutes: 45,
                    curve: core_party::EnergyCurve::Rising,
                    allow_explicit: false,
                    songs: vec![core_party::AskedSong { title: "Creep".into(), artist: "Radiohead".into() }],
                },
                provider_id: "anthropic".into(),
                model: "m".into(),
                built_ms: 5,
            }),
            ..pl::HomePlaylist::default()
        };
        let mirrored = HomePlaylist::from(original.clone());
        let PlaylistOrigin::Chat { origin } = &mirrored.origin else { panic!("a chat origin") };
        assert_eq!((origin.brief.title.as_str(), origin.brief.minutes), ("Sad 90s", 45));
        assert_eq!(pl::HomePlaylist::from(mirrored), original);
    }

    #[test]
    fn chat_settings_travel_and_default() {
        let defaults = playlist_default_settings();
        assert_eq!((defaults.chat_minutes, defaults.chat_curve), (60, EnergyCurve::Steady));
        assert!(defaults.chat_allow_explicit && defaults.chat_from_song);
        assert_eq!((defaults.chat_max_open, defaults.chat_done_notice), (3, true));
        let custom = PlaylistSettings {
            chat_minutes: 90,
            chat_curve: EnergyCurve::Arc,
            chat_allow_explicit: false,
            chat_from_song: false,
            chat_max_open: 5,
            chat_done_notice: false,
            ..defaults
        };
        assert_eq!(playlist_settings_from_json(playlist_settings_to_json(custom)), custom);
        assert_eq!(playlist_settings_from_json(r#"{"concert_shows":10}"#.into()).chat_minutes, 60);
        assert!(playlist_settings_from_json(r#"{"concert_shows":10}"#.into()).chat_from_song);
    }

    #[test]
    fn tidal_settings_travel_and_default_off() {
        let defaults = playlist_default_settings();
        assert!(!defaults.tidal_auto_save && !defaults.tidal_delete_with);
        let on = PlaylistSettings { tidal_auto_save: true, tidal_delete_with: true, ..defaults };
        assert_eq!(playlist_settings_from_json(playlist_settings_to_json(on)), on);
        assert!(!playlist_settings_from_json(r#"{"concert_shows":10}"#.into()).tidal_auto_save);
    }

    #[test]
    fn the_tidal_report_is_worded() {
        let core =
            pl::TidalSaveReport { uuid: "u".into(), created: true, saved: 24, skipped: vec!["a".into(), "b".into()] };
        let report = TidalSaveReport::new(core, Lang::Es);
        assert_eq!((report.saved, report.skipped.len()), (24, 2));
        assert_eq!(report.summary, "24 canciones guardadas en TIDAL · 2 no están en TIDAL");
    }

    #[test]
    fn every_party_word_maps_both_ways() {
        use core_party::{AgeGroup as A, EnergyCurve as C, PartyGuests as G, PartyVibe as V};
        for g in [G::Friends, G::Family, G::Coworkers, G::Mixed] {
            assert_eq!(G::from(PartyGuests::from(g)), g);
        }
        for a in [A::Kids, A::Teens, A::YoungAdults, A::Adults, A::Seniors] {
            assert_eq!(A::from(AgeGroup::from(a)), a);
        }
        for v in [V::Background, V::Chill, V::Dance, V::Singalong, V::Throwback] {
            assert_eq!(V::from(PartyVibe::from(v)), v);
        }
        for c in [C::Arc, C::Rising, C::Steady] {
            assert_eq!(C::from(EnergyCurve::from(c)), c);
        }
    }

    #[test]
    fn every_tag_maps_both_ways() {
        use pl::EntryTag as C;
        for tag in [C::Always, C::Usually, C::Sometimes, C::Encore, C::Plain] {
            assert_eq!(C::from(EntryTag::from(tag)), tag);
        }
    }

    #[test]
    fn settings_json_round_trip_and_garbage() {
        let custom = PlaylistSettings { concert_shows: 12, skip_tape: false, ..playlist_default_settings() };
        assert_eq!(playlist_settings_from_json(playlist_settings_to_json(custom)), custom);
        assert_eq!(playlist_settings_from_json("not json".into()), playlist_default_settings());
        let wild = PlaylistSettings { concert_shows: 200, search_budget: 0, ..playlist_default_settings() };
        let clamped = playlist_settings_clamped(wild);
        assert_eq!((clamped.concert_shows, clamped.search_budget), (15, 1));
        assert_eq!(playlist_settings_from_json(playlist_settings_to_json(wild)), clamped);
    }

    #[test]
    fn party_settings_travel_and_clamp() {
        let custom = PlaylistSettings {
            party_minutes: 90,
            party_curve: EnergyCurve::Steady,
            party_allow_explicit: false,
            party_max_per_artist: 5,
            ..playlist_default_settings()
        };
        assert_eq!(playlist_settings_from_json(playlist_settings_to_json(custom)), custom);
        let defaults = playlist_default_settings();
        assert_eq!((defaults.party_minutes, defaults.party_curve), (120, EnergyCurve::Arc));
        assert!(defaults.party_allow_explicit);
        assert_eq!(defaults.party_max_per_artist, 3);
        let wild = PlaylistSettings { party_minutes: 999, party_max_per_artist: 0, ..defaults };
        let clamped = playlist_settings_clamped(wild);
        assert_eq!((clamped.party_minutes, clamped.party_max_per_artist), (480, 1));
        // a settings file written before the party keys existed
        assert_eq!(playlist_settings_from_json(r#"{"concert_shows":10}"#.into()).party_minutes, 120);
    }

    #[test]
    fn request_and_progress_convert() {
        let request = ConcertRequest {
            artist: "Korn".into(),
            shows: 5,
            thresholds: playlist_default_settings().concert_thresholds,
            mode: ConcertMode::Show { show_id: "x".into() },
            popular_only: true,
        };
        let core_request = core_concert::ConcertRequest::from(request);
        assert_eq!(core_request.mode, core_concert::ConcertMode::Show { show_id: "x".into() });
        assert!(core_request.popular_only);
        assert_eq!(
            ConcertProgress::from(core_concert::ConcertProgress::Resolving { done: 2, total: 9 }),
            ConcertProgress::Resolving { done: 2, total: 9 }
        );
    }
}
