//! What a disc is called. Names come from the disc itself (CD-Text) and, only when the user has
//! turned "Look up discs on MusicBrainz" on, from MusicBrainz and the Cover Art Archive. There
//! is no key, no account and nothing paid. Nothing here fails a rip: no match is an empty list.

mod cdtext;
mod musicbrainz;

use serde::{Deserialize, Serialize};

pub use cdtext::from_cd_text;
pub use musicbrainz::{fetch_cover, MusicBrainz, MusicBrainzEndpoints};

use crate::cdtext::CdText;
use crate::settings::Sources;
use crate::toc::Toc;

/// Where a [`DiscMatch`] came from. A source this version doesn't know reads as `Manual`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    CdText,
    #[serde(rename = "musicbrainz")]
    MusicBrainz,
    #[serde(other)]
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AlbumMeta {
    pub title: String,
    pub artist: String,
    pub year: Option<u16>,
    pub genre: Option<String>,
    pub upc: Option<String>,
    pub disc_number: u8,
    pub disc_total: u8,
    /// Reserved for a Tidal lookup (always `None` today); the screen already carries the field.
    pub tidal_album_id: Option<i64>,
    /// MusicBrainz release id.
    pub mbid: Option<String>,
}

impl Default for AlbumMeta {
    fn default() -> Self {
        AlbumMeta {
            title: String::new(),
            artist: String::new(),
            year: None,
            genre: None,
            upc: None,
            disc_number: 1,
            disc_total: 1,
            tidal_album_id: None,
            mbid: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TrackMeta {
    pub number: u8,
    pub title: String,
    pub artists: Vec<String>,
    pub isrc: Option<String>,
    pub tidal_track_id: Option<i64>,
}

/// A cover the screen can show and the rip can fetch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CoverRef {
    Url { url: String },
}

/// What was found about a disc.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscMatch {
    pub source: Source,
    /// 0 to 1.
    pub confidence: f32,
    pub album: AlbumMeta,
    pub tracks: Vec<TrackMeta>,
    pub cover: Option<CoverRef>,
    pub note: Option<String>,
}

/// What is known of the disc before any lookup.
pub struct DiscFacts<'a> {
    pub toc: &'a Toc,
    pub cd_text: Option<&'a CdText>,
}

/// All the online lookups together get this long; what was found before is still returned.
const LOOKUP_BUDGET: std::time::Duration = std::time::Duration::from_secs(10);

/// Every candidate the allowed sources give, most trusted first. Never fails: a lookup that
/// can't be done (offline, 503, nothing found) just adds nothing.
pub async fn identify(facts: &DiscFacts<'_>, sources: &Sources, musicbrainz: &MusicBrainz) -> Vec<DiscMatch> {
    let mut found = Vec::new();
    if sources.cd_text {
        if let Some(text) = facts.cd_text {
            found.extend(from_cd_text(text, facts.toc));
        }
    }
    if sources.musicbrainz {
        match tokio::time::timeout(LOOKUP_BUDGET, musicbrainz.lookup(facts.toc)).await {
            Ok(matches) => found.extend(matches),
            Err(_) => tracing::debug!("the MusicBrainz lookup took too long"),
        }
    }
    found.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cdtext::CdTextTrack;
    use crate::toc::RawTocEntry;

    fn toc() -> Toc {
        let entries: Vec<RawTocEntry> =
            (1..=2).map(|n| RawTocEntry { number: n, control: 0, start_lba: u32::from(n - 1) * 1000 }).collect();
        Toc::from_entries(&entries, 2000)
    }

    fn text() -> CdText {
        CdText {
            title: Some("Album".into()),
            artist: Some("Artist".into()),
            tracks: vec![CdTextTrack { number: 1, title: Some("One".into()), artist: None }],
            ..CdText::default()
        }
    }

    #[tokio::test]
    async fn without_the_musicbrainz_switch_nothing_goes_online() {
        // an endpoint that can't be reached would make the test slow or fail if it were used
        let endpoints = MusicBrainzEndpoints { api: "http://127.0.0.1:9".into(), covers: "http://127.0.0.1:9".into() };
        let mb = MusicBrainz::with_endpoints(reqwest::Client::new(), endpoints);
        let toc = toc();
        let facts = DiscFacts { toc: &toc, cd_text: Some(&text()) };
        let found = identify(&facts, &Sources { cd_text: true, musicbrainz: false }, &mb).await;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].source, Source::CdText);
        let none = identify(&facts, &Sources { cd_text: false, musicbrainz: false }, &mb).await;
        assert!(none.is_empty());
    }

    #[tokio::test]
    async fn an_unreachable_service_still_returns_the_disc_s_own_names() {
        let endpoints = MusicBrainzEndpoints { api: "http://127.0.0.1:9".into(), covers: "http://127.0.0.1:9".into() };
        let mb = MusicBrainz::with_endpoints(reqwest::Client::new(), endpoints);
        let toc = toc();
        let facts = DiscFacts { toc: &toc, cd_text: Some(&text()) };
        let found = identify(&facts, &Sources { cd_text: true, musicbrainz: true }, &mb).await;
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn an_unknown_source_reads_as_manual_and_names_match_the_screen() {
        assert_eq!(serde_json::from_str::<Source>("\"tidal\"").unwrap(), Source::Manual);
        assert_eq!(serde_json::to_string(&Source::MusicBrainz).unwrap(), "\"musicbrainz\"");
        assert_eq!(serde_json::to_string(&Source::CdText).unwrap(), "\"cdText\"");
        let cover = serde_json::to_value(CoverRef::Url { url: "https://x/y.jpg".into() }).unwrap();
        assert_eq!(cover, serde_json::json!({"kind": "url", "url": "https://x/y.jpg"}));
    }

    #[test]
    fn a_match_from_the_screen_with_missing_fields_still_reads() {
        let json = r#"{"source":"manual","confidence":1,"album":{"title":"A","artist":"B"},
            "tracks":[{"number":1,"title":"T"}],"cover":null,"note":null}"#;
        let parsed: DiscMatch = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.album.disc_number, 1);
        assert!(parsed.tracks[0].artists.is_empty());
    }
}
