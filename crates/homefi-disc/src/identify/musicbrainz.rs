//! MusicBrainz (free, no key) and the Cover Art Archive, only when the user turned the lookup
//! on. MusicBrainz asks for a descriptive User-Agent with a way to reach us and at most one
//! request per second; both are enforced here.

use std::time::Duration;

use reqwest::StatusCode;
use serde::Deserialize;
use tokio::sync::Mutex;
use tokio::time::Instant;

use super::{AlbumMeta, CoverRef, DiscMatch, Source, TrackMeta};
use crate::toc::Toc;

const DEFAULT_API: &str = "https://musicbrainz.org";
const DEFAULT_COVERS: &str = "https://coverartarchive.org";
/// MusicBrainz's rule for anonymous clients.
const MIN_REQUEST_SPACING: Duration = Duration::from_secs(1);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const COVER_TIMEOUT: Duration = Duration::from_secs(15);
/// Covers of the archive are a few hundred kB; anything bigger is not a cover.
const MAX_COVER_BYTES: usize = 10 * 1024 * 1024;
const MAX_CANDIDATES: usize = 5;
/// The disc id names this exact pressing.
const CONFIDENCE_DISC_ID: f32 = 0.95;
/// Only the table of contents is close: could be another pressing of the same album.
const CONFIDENCE_TOC: f32 = 0.6;
/// Cover size asked from the archive (it has 250, 500 and 1200).
const COVER_SIZE: u16 = 500;
const INCLUDES: &str = "artist-credits+recordings+release-groups+genres+isrcs";
const APP: &str = "HomeFi";

/// Where the services are; tests point them at a local server.
#[derive(Debug, Clone)]
pub struct MusicBrainzEndpoints {
    pub api: String,
    pub covers: String,
}

impl Default for MusicBrainzEndpoints {
    fn default() -> Self {
        MusicBrainzEndpoints { api: DEFAULT_API.into(), covers: DEFAULT_COVERS.into() }
    }
}

/// "HomeFi/1.5.2 ( https://github.com/... )": the app, its version and where to reach it.
pub fn user_agent() -> String {
    format!("{APP}/{} ( {} )", env!("CARGO_PKG_VERSION"), env!("CARGO_PKG_REPOSITORY"))
}

/// Lets a request through at most once per `spacing`; later callers wait their turn.
struct RateLimiter {
    spacing: Duration,
    next: Mutex<Instant>,
}

impl RateLimiter {
    fn new(spacing: Duration) -> Self {
        RateLimiter { spacing, next: Mutex::new(Instant::now()) }
    }

    async fn wait(&self) {
        let mut next = self.next.lock().await;
        let now = Instant::now();
        if *next > now {
            tokio::time::sleep_until(*next).await;
        }
        *next = (*next).max(now) + self.spacing;
    }
}

enum Reply {
    Releases(Vec<Release>),
    /// MusicBrainz doesn't know this disc id.
    NotFound,
    /// Offline, throttled, broken answer: asking again in another way won't help.
    Failed,
}

/// One of these for the whole app: the limiter is per client, not per call.
pub struct MusicBrainz {
    http: reqwest::Client,
    endpoints: MusicBrainzEndpoints,
    limiter: RateLimiter,
}

impl MusicBrainz {
    pub fn new(http: reqwest::Client) -> Self {
        Self::with_endpoints(http, MusicBrainzEndpoints::default())
    }

    pub fn with_endpoints(http: reqwest::Client, endpoints: MusicBrainzEndpoints) -> Self {
        MusicBrainz { http, endpoints, limiter: RateLimiter::new(MIN_REQUEST_SPACING) }
    }

    /// Releases for this disc: exact pressings first, then looser matches by table of contents.
    pub async fn lookup(&self, toc: &Toc) -> Vec<DiscMatch> {
        let disc_id = toc.musicbrainz_id();
        if disc_id.is_empty() {
            return Vec::new();
        }
        let exact = format!("{}/ws/2/discid/{disc_id}?inc={INCLUDES}&fmt=json", self.endpoints.api);
        let found = match self.releases(&exact).await {
            Reply::Releases(releases) => self.matches(&releases, toc, Some(&disc_id)),
            Reply::Failed => return Vec::new(),
            Reply::NotFound => Vec::new(),
        };
        if !found.is_empty() {
            return found;
        }
        // not a known pressing (or none that fits): releases with the same layout
        let fuzzy = format!(
            "{}/ws/2/discid/-?toc={}&cdstubs=no&inc={INCLUDES}&fmt=json",
            self.endpoints.api,
            toc.musicbrainz_toc()
        );
        match self.releases(&fuzzy).await {
            Reply::Releases(releases) => self.matches(&releases, toc, None),
            Reply::NotFound | Reply::Failed => Vec::new(),
        }
    }

    /// One request, with the answer reduced to what matters.
    async fn releases(&self, url: &str) -> Reply {
        self.limiter.wait().await;
        let response = self
            .http
            .get(url)
            .header(reqwest::header::USER_AGENT, user_agent())
            .header(reqwest::header::ACCEPT, "application/json")
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await
            .inspect_err(|error| tracing::debug!(%error, "musicbrainz request failed"));
        let Ok(response) = response else { return Reply::Failed };
        match response.status() {
            StatusCode::OK => {}
            StatusCode::NOT_FOUND => return Reply::NotFound,
            status => {
                tracing::debug!(%status, "musicbrainz answered no");
                return Reply::Failed;
            }
        }
        match response.json::<Lookup>().await {
            Ok(body) => Reply::Releases(body.releases),
            Err(error) => {
                tracing::debug!(%error, "musicbrainz reply");
                Reply::Failed
            }
        }
    }

    fn matches(&self, releases: &[Release], toc: &Toc, disc_id: Option<&str>) -> Vec<DiscMatch> {
        let audio_tracks = toc.audio_tracks().count();
        releases
            .iter()
            .filter_map(|release| {
                let (position, medium) = release.media.iter().enumerate().find(|(_, medium)| {
                    let same_disc = disc_id.is_none_or(|id| medium.discs.iter().any(|d| d.id == id));
                    same_disc && medium.tracks.len() == audio_tracks
                })?;
                Some(self.to_match(release, medium, position, toc, disc_id.is_some()))
            })
            .take(MAX_CANDIDATES)
            .collect()
    }

    fn to_match(&self, release: &Release, medium: &Medium, position: usize, toc: &Toc, exact: bool) -> DiscMatch {
        let artist = credit_text(&release.artist_credit);
        let numbers: Vec<u8> = toc.audio_tracks().map(|t| t.number).collect();
        let tracks = medium
            .tracks
            .iter()
            .zip(numbers)
            .map(|(track, number)| {
                let credit = if track.artist_credit.is_empty() { &release.artist_credit } else { &track.artist_credit };
                let names: Vec<String> = credit.iter().map(|c| c.artist_name().to_owned()).collect();
                TrackMeta {
                    number,
                    title: track.title.clone(),
                    artists: dedupe(names),
                    isrc: track.recording.as_ref().and_then(|r| r.isrcs.first().cloned()),
                    ..TrackMeta::default()
                }
            })
            .collect();
        let cover = release.cover_art_archive.as_ref().is_some_and(|c| c.front).then(|| CoverRef::Url {
            url: format!("{}/release/{}/front-{COVER_SIZE}", self.endpoints.covers, release.id),
        });
        DiscMatch {
            source: Source::MusicBrainz,
            confidence: if exact { CONFIDENCE_DISC_ID } else { CONFIDENCE_TOC },
            album: AlbumMeta {
                title: release.title.clone(),
                artist,
                year: release.date.as_deref().and_then(year_of),
                genre: top_genre(&release.genres),
                upc: release.barcode.clone().filter(|b| !b.is_empty()),
                disc_number: u8::try_from(medium.position.max(position as u32 + 1)).unwrap_or(1),
                disc_total: u8::try_from(release.media.len()).unwrap_or(1).max(1),
                mbid: Some(release.id.clone()),
                ..AlbumMeta::default()
            },
            tracks,
            cover,
            note: (!exact).then(|| "Matched by the disc's layout, not its exact id".to_owned()),
        }
    }
}

/// Downloads a cover; `None` when it can't be had (it is never worth failing a rip for).
pub async fn fetch_cover(http: &reqwest::Client, url: &str) -> Option<Vec<u8>> {
    let response = http
        .get(url)
        .header(reqwest::header::USER_AGENT, user_agent())
        .timeout(COVER_TIMEOUT)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    if response.content_length().is_some_and(|len| len > MAX_COVER_BYTES as u64) {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    (!bytes.is_empty() && bytes.len() <= MAX_COVER_BYTES).then(|| bytes.to_vec())
}

fn year_of(date: &str) -> Option<u16> {
    date.split('-').next()?.parse().ok().filter(|y| *y > 0)
}

fn dedupe(names: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(names.len());
    for name in names {
        if !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

fn credit_text(credit: &[ArtistCredit]) -> String {
    credit.iter().map(|c| format!("{}{}", c.display_name(), c.joinphrase.as_deref().unwrap_or(""))).collect()
}

/// The genre voted most, with a capital letter on every word.
fn top_genre(genres: &[Genre]) -> Option<String> {
    let best = genres.iter().max_by_key(|g| g.count)?;
    let name: Vec<String> = best
        .name
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
        })
        .collect();
    Some(name.join(" ")).filter(|n| !n.is_empty())
}

// ---- the parts of the reply we read ----------------------------------------------------------

#[derive(Debug, Deserialize)]
struct Lookup {
    #[serde(default)]
    releases: Vec<Release>,
}

#[derive(Debug, Deserialize)]
struct Release {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    date: Option<String>,
    #[serde(default)]
    barcode: Option<String>,
    #[serde(default, rename = "artist-credit")]
    artist_credit: Vec<ArtistCredit>,
    #[serde(default)]
    media: Vec<Medium>,
    #[serde(default)]
    genres: Vec<Genre>,
    #[serde(default, rename = "cover-art-archive")]
    cover_art_archive: Option<CoverArtArchive>,
}

#[derive(Debug, Deserialize)]
struct Medium {
    #[serde(default)]
    position: u32,
    #[serde(default)]
    discs: Vec<Disc>,
    #[serde(default)]
    tracks: Vec<Track>,
}

#[derive(Debug, Deserialize)]
struct Disc {
    id: String,
}

#[derive(Debug, Deserialize)]
struct Track {
    #[serde(default)]
    title: String,
    #[serde(default, rename = "artist-credit")]
    artist_credit: Vec<ArtistCredit>,
    #[serde(default)]
    recording: Option<Recording>,
}

#[derive(Debug, Deserialize)]
struct Recording {
    #[serde(default)]
    isrcs: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ArtistCredit {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    joinphrase: Option<String>,
    #[serde(default)]
    artist: Option<ArtistName>,
}

#[derive(Debug, Deserialize)]
struct ArtistName {
    #[serde(default)]
    name: String,
}

impl ArtistCredit {
    /// How the credit is printed (can differ from the artist's own name).
    fn display_name(&self) -> &str {
        self.name.as_deref().or_else(|| self.artist.as_ref().map(|a| a.name.as_str())).unwrap_or_default()
    }

    fn artist_name(&self) -> &str {
        self.artist.as_ref().map(|a| a.name.as_str()).filter(|n| !n.is_empty()).unwrap_or_else(|| self.display_name())
    }
}

#[derive(Debug, Deserialize)]
struct Genre {
    #[serde(default)]
    name: String,
    #[serde(default)]
    count: i64,
}

#[derive(Debug, Deserialize)]
struct CoverArtArchive {
    #[serde(default)]
    front: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toc::{RawTocEntry, Toc};
    use wiremock::matchers::{header, method, path, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn toc() -> Toc {
        let entries: Vec<RawTocEntry> =
            (1..=3).map(|n| RawTocEntry { number: n, control: 0, start_lba: u32::from(n - 1) * 20_000 }).collect();
        Toc::from_entries(&entries, 60_000)
    }

    fn release(disc_id: &str) -> serde_json::Value {
        serde_json::json!({
            "id": "5a3c1b7e-0000-4000-8000-000000000000",
            "title": "Korn",
            "date": "1994-10-11",
            "barcode": "074646672229",
            "artist-credit": [{"name": "Korn", "joinphrase": "", "artist": {"name": "Korn"}}],
            "genres": [{"name": "nu metal", "count": 9}, {"name": "rock", "count": 2}],
            "cover-art-archive": {"front": true},
            "media": [{
                "position": 1,
                "discs": [{"id": disc_id}],
                "tracks": [
                    {"title": "Blind", "recording": {"isrcs": ["USSM19400001"]}},
                    {"title": "Ball Tongue", "artist-credit": [{"name": "Korn", "artist": {"name": "Korn"}}, {"name": "Guest", "artist": {"name": "Guest"}}]},
                    {"title": "Need To"}
                ]
            }]
        })
    }

    fn client(server: &MockServer) -> MusicBrainz {
        let endpoints = MusicBrainzEndpoints { api: server.uri(), covers: format!("{}/covers", server.uri()) };
        MusicBrainz::with_endpoints(reqwest::Client::new(), endpoints)
    }

    #[tokio::test]
    async fn an_exact_disc_id_gives_the_release_with_the_app_user_agent() {
        let server = MockServer::start().await;
        let toc = toc();
        let reply = serde_json::json!({ "releases": [release(&toc.musicbrainz_id())] });
        Mock::given(method("GET"))
            .and(path(format!("/ws/2/discid/{}", toc.musicbrainz_id())))
            .and(header("user-agent", user_agent().as_str()))
            .respond_with(ResponseTemplate::new(200).set_body_json(reply))
            .expect(1)
            .mount(&server)
            .await;

        let found = client(&server).lookup(&toc).await;
        assert_eq!(found.len(), 1);
        let found = &found[0];
        assert_eq!(found.source, Source::MusicBrainz);
        assert!(found.confidence > 0.9);
        assert_eq!((found.album.title.as_str(), found.album.artist.as_str()), ("Korn", "Korn"));
        assert_eq!(found.album.year, Some(1994));
        assert_eq!(found.album.genre.as_deref(), Some("Nu Metal"));
        assert_eq!(found.album.upc.as_deref(), Some("074646672229"));
        assert_eq!(found.tracks.len(), 3);
        assert_eq!(found.tracks[0].isrc.as_deref(), Some("USSM19400001"));
        assert_eq!(found.tracks[1].artists, ["Korn", "Guest"]);
        assert_eq!(found.tracks[2].artists, ["Korn"], "falls back to the album's artist");
        assert_eq!(
            found.cover,
            Some(CoverRef::Url {
                url: format!("{}/covers/release/5a3c1b7e-0000-4000-8000-000000000000/front-500", server.uri())
            })
        );
        assert!(user_agent().starts_with("HomeFi/") && user_agent().contains("https://"));
    }

    #[tokio::test]
    async fn an_unknown_id_falls_back_to_the_layout_with_less_confidence() {
        let server = MockServer::start().await;
        let toc = toc();
        Mock::given(method("GET"))
            .and(path_regex(r"^/ws/2/discid/-$"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "releases": [release("other-disc-id")] })),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(404)).mount(&server).await;

        let found = client(&server).lookup(&toc).await;
        assert_eq!(found.len(), 1);
        assert!(found[0].confidence < 0.7);
        assert!(found[0].note.is_some());
    }

    #[tokio::test]
    async fn a_release_with_another_number_of_tracks_is_not_a_match() {
        let server = MockServer::start().await;
        let toc = toc();
        let mut other = release(&toc.musicbrainz_id());
        other["media"][0]["tracks"].as_array_mut().unwrap().pop();
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "releases": [other] })))
            .mount(&server)
            .await;
        assert!(client(&server).lookup(&toc).await.is_empty());
    }

    #[tokio::test]
    async fn a_busy_service_or_garbage_gives_nothing_not_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(503)).mount(&server).await;
        assert!(client(&server).lookup(&toc()).await.is_empty());

        let garbage = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>"))
            .mount(&garbage)
            .await;
        assert!(client(&garbage).lookup(&toc()).await.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn requests_are_one_second_apart() {
        let limiter = RateLimiter::new(MIN_REQUEST_SPACING);
        let started = Instant::now();
        limiter.wait().await;
        assert!(started.elapsed() < Duration::from_millis(10), "the first goes at once");
        limiter.wait().await;
        limiter.wait().await;
        assert!(started.elapsed() >= Duration::from_secs(2));
        assert!(started.elapsed() < Duration::from_millis(2100));
    }

    #[tokio::test]
    async fn the_cover_is_fetched_with_the_user_agent_and_junk_is_ignored() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/covers/ok"))
            .and(header("user-agent", user_agent().as_str()))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![0xFF, 0xD8, 0xFF, 0xD9]))
            .mount(&server)
            .await;
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(404)).mount(&server).await;
        let http = reqwest::Client::new();
        assert_eq!(
            fetch_cover(&http, &format!("{}/covers/ok", server.uri())).await,
            Some(vec![0xFF, 0xD8, 0xFF, 0xD9])
        );
        assert_eq!(fetch_cover(&http, &format!("{}/covers/missing", server.uri())).await, None);
    }
}
