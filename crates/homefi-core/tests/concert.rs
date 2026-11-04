//! "Get ready for a concert" end to end against mock servers: the AI's two
//! phases (Anthropic and Gemini wire formats), Tidal's search and top tracks,
//! the fallbacks and `rearrange`. Nothing here touches the real network.

use std::sync::{Arc, Mutex};

use homefi_core::ai::{AiConfig, AiProvider};
use homefi_core::api::{TidalApi, TidalClient};
use homefi_core::concert::{
    build, rearrange, BuildContext, ConcertBuilt, ConcertMode, ConcertProgress, ConcertRequest, ConcertThresholds,
};
use homefi_core::humane::Lang;
use homefi_core::playlist::{EntryTag, HomePlaylist, LibraryIndex, PlaylistOrigin, PlaylistSettings, SetlistSource};
use homefi_core::Error;
use serde_json::{json, Value};
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const PHASE1: &str = include_str!("fixtures/concert/phase1_research.json");
const PHASE2: &str = include_str!("fixtures/concert/phase2_submit_shows.json");
const PHASE2_ONE: &str = include_str!("fixtures/concert/phase2_one_show.json");
const GEMINI_PHASE1: &str = include_str!("fixtures/concert/gemini_phase1_grounded.json");
const GEMINI_PHASE2: &str = include_str!("fixtures/concert/gemini_phase2_submit_shows.json");
const KEY: &str = "sk-test";
const ARTISTS: [&str; 2] = ["Chris Isaak", "Korn"];

fn json_ok(fixture: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(serde_json::from_str::<Value>(fixture).unwrap())
}

/// Tidal's `/v1/search`: the artist when the query is just its name, otherwise
/// one studio track whose title is the rest of the query. "Made Up Song" is unknown.
struct TidalSearch;

impl Respond for TidalSearch {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let query =
            request.url.query_pairs().find(|(k, _)| k == "query").map(|(_, v)| v.into_owned()).unwrap_or_default();
        let page =
            |items: Vec<Value>| json!({ "limit": 50, "offset": 0, "totalNumberOfItems": items.len(), "items": items });
        if query == "Korn" {
            let artists = page(vec![json!({ "id": 7, "name": "Korn" })]);
            return ResponseTemplate::new(200).set_body_json(json!({ "artists": artists }));
        }
        let artist = ARTISTS.iter().find(|a| query.starts_with(*a)).copied().unwrap_or("Korn");
        let title = query.strip_prefix(artist).unwrap_or(&query).trim();
        let tracks = if title.is_empty() || title == "Made Up Song" { Vec::new() } else { vec![track(title, artist)] };
        ResponseTemplate::new(200).set_body_json(json!({ "tracks": page(tracks) }))
    }
}

fn track(title: &str, artist: &str) -> Value {
    let id: i64 = title.bytes().map(i64::from).sum::<i64>() + 1000;
    json!({
        "id": id, "title": title, "duration": 200, "allowStreaming": true, "explicit": false, "popularity": 50,
        "isrc": format!("TEST{id}"),
        "artist": { "id": 7, "name": artist, "type": "MAIN" },
        "artists": [{ "id": 7, "name": artist, "type": "MAIN" }],
        "album": { "id": 5, "title": "Album", "cover": null }
    })
}

async fn mount_tidal(server: &MockServer) {
    Mock::given(method("GET")).and(path("/v1/search")).respond_with(TidalSearch).mount(server).await;
    let top = json!({ "limit": 50, "offset": 0, "totalNumberOfItems": 3, "items": [
        track("Blind", "Korn"), track("Freak On A Leash", "Korn"), track("Falling Away From Me", "Korn")
    ] });
    Mock::given(method("GET"))
        .and(path("/v1/artists/7/toptracks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(top))
        .mount(server)
        .await;
}

async fn mount_anthropic(server: &MockServer, phase2: &str) {
    Mock::given(method("POST"))
        .and(path("/messages"))
        .and(body_string_contains("web_search_20250305"))
        .respond_with(json_ok(PHASE1))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .and(body_string_contains("submit_shows"))
        .respond_with(json_ok(phase2))
        .mount(server)
        .await;
}

fn tidal_api(server: &MockServer) -> TidalApi {
    let client =
        TidalClient::new("token").with_base_urls(format!("{}/v1", server.uri()), format!("{}/v2", server.uri()));
    TidalApi::new(Arc::new(client), "1", "US")
}

fn ai(provider: AiProvider, server: &MockServer) -> AiConfig {
    AiConfig::new(provider, "model-x", Some(KEY.into())).with_base_url(server.uri())
}

fn request() -> ConcertRequest {
    ConcertRequest { artist: "Korn".into(), ..ConcertRequest::default() }
}

fn progress_log() -> (Arc<Mutex<Vec<ConcertProgress>>>, impl Fn(ConcertProgress) + Send + Sync) {
    let log = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    (log, move |p| sink.lock().unwrap().push(p))
}

struct Fixture {
    settings: PlaylistSettings,
    library: LibraryIndex,
}

impl Fixture {
    fn new() -> Self {
        Self { settings: PlaylistSettings::default(), library: LibraryIndex::default() }
    }

    fn ctx<'a>(&'a self, ai: Option<&'a AiConfig>, api: Option<&'a TidalApi>, lang: Lang) -> BuildContext<'a> {
        BuildContext { settings: &self.settings, ai, api, library: &self.library, lang }
    }
}

async fn run(
    fx: &Fixture,
    req: &ConcertRequest,
    ai: Option<&AiConfig>,
    api: Option<&TidalApi>,
) -> homefi_core::Result<ConcertBuilt> {
    build(req, &fx.ctx(ai, api, Lang::En), &|_| {}).await
}

fn titles(playlist: &HomePlaylist) -> Vec<&str> {
    playlist.entries.iter().map(|e| e.title.as_str()).collect()
}

fn origin(playlist: &HomePlaylist) -> &homefi_core::playlist::ConcertOrigin {
    let PlaylistOrigin::Concert(o) = &playlist.origin else { panic!("a concert playlist") };
    o
}

async fn requests(server: &MockServer) -> Vec<Request> {
    server.received_requests().await.unwrap()
}

fn searches(all: &[Request]) -> Vec<String> {
    all.iter()
        .filter(|r| r.url.path() == "/v1/search")
        .filter_map(|r| r.url.query_pairs().find(|(k, _)| k == "query").map(|(_, v)| v.into_owned()))
        .collect()
}

fn count_ai(all: &[Request]) -> usize {
    all.iter().filter(|r| r.method.as_str() == "POST").count()
}

#[tokio::test]
async fn the_happy_path_builds_the_tour_set() {
    let server = MockServer::start().await;
    mount_anthropic(&server, PHASE2).await;
    mount_tidal(&server).await;
    let (config, api, fx) = (ai(AiProvider::Anthropic, &server), tidal_api(&server), Fixture::new());
    let (log, progress) = progress_log();

    let built = build(&request(), &fx.ctx(Some(&config), Some(&api), Lang::En), &progress).await.unwrap();

    assert_eq!(built.fallback_note, None);
    let p = &built.playlist;
    let o = origin(p);
    assert_eq!(o.shows.len(), 3, "future, short and duplicate shows are dropped");
    assert_eq!(
        o.shows.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        ["2026-10-24-0", "2026-10-22-1", "2026-10-20-2"]
    );
    assert_eq!(o.source, SetlistSource::Ai { provider_id: "anthropic".into(), model: "model-x".into() });
    assert_eq!(p.name, "Korn · Tour setlist");
    assert_eq!(p.description, "From 3 recent shows");

    // order = median position; tapes are dropped by default
    assert_eq!(titles(p).first().copied(), Some("Blind"));
    assert_eq!(titles(p).last().copied(), Some("Wicked Game"));
    assert!(!titles(p).contains(&"Intro"));
    let tag_of = |t: &str| p.entries.iter().find(|e| e.title == t).map(|e| e.tag);
    assert_eq!(tag_of("Blind"), Some(EntryTag::Always));
    assert_eq!(tag_of("Got the Life"), Some(EntryTag::Usually));
    assert_eq!(tag_of("Falling Away From Me"), Some(EntryTag::Sometimes));
    assert_eq!(tag_of("Wicked Game"), Some(EntryTag::Encore));
    let wicked = p.entries.iter().find(|e| e.title == "Wicked Game").unwrap();
    assert_eq!(wicked.note, "Played in 3 of 3 shows · encore · Chris Isaak cover");
    assert_eq!(wicked.artist, "Chris Isaak");

    assert_eq!(
        p.sources.iter().map(|s| s.url.as_str()).collect::<Vec<_>>(),
        ["https://www.example.com/korn-2026-10-24", "https://news.example.org/korn-tour"]
    );
    assert_eq!(p.search_widget_html, None);

    let all = requests(&server).await;
    assert!(
        searches(&all).contains(&"Chris Isaak Wicked Game".to_owned()),
        "a cover is searched under its original artist"
    );
    assert!(!searches(&all).contains(&"Korn Wicked Game".to_owned()));
    assert!(all.iter().all(|r| (r.method.as_str() == "POST" && r.url.path() == "/messages")
        || (r.method.as_str() == "GET" && r.url.path().starts_with("/v1/"))));
    assert_eq!(count_ai(&all), 2);

    let seen = log.lock().unwrap().clone();
    assert_eq!(seen[0], ConcertProgress::Searching { with_ai: true });
    assert_eq!(seen[1], ConcertProgress::ShowsFound { count: 3 });
    let total = p.entries.len() + p.unmatched.len();
    let mut dones: Vec<usize> = seen[2..]
        .iter()
        .map(|p| match p {
            ConcertProgress::Resolving { done, total: t } => {
                assert_eq!(*t, total);
                *done
            }
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    dones.sort_unstable();
    assert_eq!(dones, (1..=total).collect::<Vec<_>>());
}

#[tokio::test]
async fn tapes_and_covers_follow_the_settings() {
    let server = MockServer::start().await;
    mount_anthropic(&server, PHASE2).await;
    mount_tidal(&server).await;
    let (config, api) = (ai(AiProvider::Anthropic, &server), tidal_api(&server));
    let fx = Fixture {
        settings: PlaylistSettings { skip_tape: false, covers_original_artist: false, ..PlaylistSettings::default() },
        library: LibraryIndex::default(),
    };
    let built = run(&fx, &request(), Some(&config), Some(&api)).await.unwrap();
    assert!(titles(&built.playlist).contains(&"Intro"), "tapes are kept when skip_tape is off");
    assert!(searches(&requests(&server).await).contains(&"Korn Wicked Game".to_owned()));
}

#[tokio::test]
async fn too_few_shows_fall_back_to_the_most_played() {
    let server = MockServer::start().await;
    mount_anthropic(&server, PHASE2_ONE).await;
    mount_tidal(&server).await;
    let (config, api, fx) = (ai(AiProvider::Anthropic, &server), tidal_api(&server), Fixture::new());
    let built = run(&fx, &request(), Some(&config), Some(&api)).await.unwrap();
    assert_eq!(
        built.fallback_note.as_deref(),
        Some("Your AI didn't find enough recent setlists for Korn, so these are their most played songs on Tidal.")
    );
    assert_eq!(origin(&built.playlist).source, SetlistSource::TidalPopular);
    assert!(origin(&built.playlist).shows.is_empty());
    assert_eq!(titles(&built.playlist), ["Blind", "Freak On A Leash", "Falling Away From Me"]);
    assert_eq!(built.playlist.name, "Korn · Most played");
}

#[tokio::test]
async fn a_provider_that_cannot_search_is_never_called() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(500)).mount(&server).await;
    mount_tidal(&server).await;
    let (config, api, fx) = (ai(AiProvider::Mistral, &server), tidal_api(&server), Fixture::new());
    let built = run(&fx, &request(), Some(&config), Some(&api)).await.unwrap();
    assert_eq!(count_ai(&requests(&server).await), 0);
    let note = built.fallback_note.unwrap();
    assert!(note.starts_with("Mistral can't search the web"), "{note}");
}

#[tokio::test]
async fn an_ai_failure_falls_back_without_leaking_the_key() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(401).set_body_json(json!({ "error": { "message": format!("bad key {KEY}") } })),
        )
        .mount(&server)
        .await;
    mount_tidal(&server).await;
    let (config, api, fx) = (ai(AiProvider::Anthropic, &server), tidal_api(&server), Fixture::new());
    let built = run(&fx, &request(), Some(&config), Some(&api)).await.unwrap();
    let note = built.fallback_note.unwrap();
    assert!(note.contains("key isn't valid"), "{note}");
    assert!(note.ends_with("These are Korn's most played songs on Tidal instead."), "{note}");
    assert!(!note.contains(KEY));
    assert_eq!(origin(&built.playlist).source, SetlistSource::TidalPopular);
}

#[tokio::test]
async fn nothing_to_use_is_a_worded_error() {
    let fx = Fixture::new();
    let err = build(&request(), &fx.ctx(None, None, Lang::Es), &|_| {}).await.unwrap_err();
    match err {
        Error::Playlist(text) => {
            assert_eq!(text, "Conecta Tidal o configura una IA con búsqueda web para armar esta playlist.");
        }
        other => panic!("unexpected {other:?}"),
    }
    let blank = ConcertRequest { artist: "  ".into(), ..ConcertRequest::default() };
    let err = build(&blank, &fx.ctx(None, None, Lang::En), &|_| {}).await.unwrap_err();
    assert_eq!(err.to_string(), "Type the artist's name.");
}

#[tokio::test]
async fn no_ai_uses_tidal_and_says_so() {
    let server = MockServer::start().await;
    mount_tidal(&server).await;
    let (api, fx) = (tidal_api(&server), Fixture::new());
    let built = run(&fx, &request(), None, Some(&api)).await.unwrap();
    assert!(built.fallback_note.unwrap().starts_with("No AI is set up"));
    let missing = ConcertRequest { artist: "Nobody".into(), ..ConcertRequest::default() };
    // the mock answers a track search for any other name, never an artist
    let err = run(&fx, &missing, None, Some(&api)).await.unwrap_err();
    assert_eq!(err.to_string(), "Tidal has no artist called “Nobody”.");
}

#[tokio::test]
async fn gemini_keeps_the_search_widget() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/models/model-x:generateContent"))
        .and(body_string_contains("google_search"))
        .respond_with(json_ok(GEMINI_PHASE1))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/models/model-x:generateContent"))
        .and(body_string_contains("submit_shows"))
        .respond_with(json_ok(GEMINI_PHASE2))
        .mount(&server)
        .await;
    mount_tidal(&server).await;
    let (config, api, fx) = (ai(AiProvider::Gemini, &server), tidal_api(&server), Fixture::new());
    let built = run(&fx, &request(), Some(&config), Some(&api)).await.unwrap();
    assert!(built.playlist.search_widget_html.as_deref().unwrap().contains("korn setlist"));
    assert_eq!(built.playlist.sources.len(), 1);
}

#[tokio::test]
async fn popular_only_never_calls_the_ai() {
    let server = MockServer::start().await;
    mount_tidal(&server).await;
    let (config, api, fx) = (ai(AiProvider::Anthropic, &server), tidal_api(&server), Fixture::new());
    let req = ConcertRequest { popular_only: true, ..request() };
    let built = run(&fx, &req, Some(&config), Some(&api)).await.unwrap();
    assert_eq!(built.fallback_note, None);
    assert_eq!(count_ai(&requests(&server).await), 0);
    assert_eq!(origin(&built.playlist).source, SetlistSource::TidalPopular);
}

#[tokio::test]
async fn without_tidal_the_ai_result_says_to_connect_it() {
    let server = MockServer::start().await;
    mount_anthropic(&server, PHASE2).await;
    let (config, fx) = (ai(AiProvider::Anthropic, &server), Fixture::new());
    let built = run(&fx, &request(), Some(&config), None).await.unwrap();
    assert_eq!(built.fallback_note.as_deref(), Some("Connect Tidal to find the songs that aren't in your library."));
    assert!(built.playlist.entries.is_empty());
    assert!(!built.playlist.unmatched.is_empty());
}

#[tokio::test]
async fn rearrange_changes_the_view_without_asking_the_ai() {
    let server = MockServer::start().await;
    mount_anthropic(&server, PHASE2).await;
    mount_tidal(&server).await;
    let (config, api, fx) = (ai(AiProvider::Anthropic, &server), tidal_api(&server), Fixture::new());
    let loose = ConcertThresholds { always_pct: 60, ..ConcertThresholds::default() };
    let built = run(&fx, &ConcertRequest { thresholds: loose, ..request() }, Some(&config), Some(&api)).await.unwrap();
    let old = built.playlist;
    let always = |p: &HomePlaylist| p.entries.iter().filter(|e| e.tag == EntryTag::Always).count();
    let posts = count_ai(&requests(&server).await);
    let ctx = fx.ctx(Some(&config), Some(&api), Lang::En);

    let one_show = ConcertRequest { mode: ConcertMode::Show { show_id: "2026-10-24-0".into() }, ..request() };
    let shown = rearrange(&old, &one_show, &ctx, &|_| {}).await.unwrap();
    assert_eq!(titles(&shown), ["Blind", "Got the Life", "Falling Away From Me", "Freak On A Leash", "Wicked Game"]);
    assert_eq!((&shown.id, shown.created_ms), (&old.id, old.created_ms));
    assert_eq!(shown.name, "Korn · Mexico City, 2026-10-24");
    assert_eq!(shown.sources, old.sources);
    assert_eq!(origin(&shown).shows, origin(&old).shows, "the stored shows are kept");

    let strict = ConcertRequest { thresholds: ConcertThresholds { always_pct: 100, ..loose }, ..request() };
    let tour = rearrange(&old, &strict, &ctx, &|_| {}).await.unwrap();
    assert!(always(&tour) < always(&old), "{} vs {}", always(&tour), always(&old));
    assert_eq!(origin(&tour).thresholds.always_pct, 100);

    let unknown = ConcertRequest { mode: ConcertMode::Show { show_id: "nope".into() }, ..request() };
    assert!(matches!(rearrange(&old, &unknown, &ctx, &|_| {}).await, Err(Error::Playlist(_))));
    let no_shows = HomePlaylist { origin: PlaylistOrigin::default(), ..old.clone() };
    assert!(matches!(rearrange(&no_shows, &request(), &ctx, &|_| {}).await, Err(Error::Playlist(_))));

    assert_eq!(count_ai(&requests(&server).await), posts, "rearrange makes no AI call");
}
