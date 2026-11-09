//! HomeFi playlists: the JSON store, the resolver and Tidal's most played,
//! against a mock Tidal server. Nothing here writes to Tidal: every test checks
//! that only `GET` requests were made.

use std::sync::{Arc, Mutex};

use homefi_core::api::{TidalApi, TidalClient};
use homefi_core::concert::most_played;
use homefi_core::playlist::{
    EntryTag, HomePlaylist, LibraryIndex, LibraryRef, PlaylistStore, Resolver, SongCandidate, MIN_CONFIDENCE,
};
use serde_json::json;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SEARCH_FIXTURE: &str = include_str!("fixtures/playlists/tidal_search_freak.json");

fn api(server: &MockServer) -> TidalApi {
    let base = server.uri();
    let client = TidalClient::new("token").with_base_urls(format!("{base}/v1"), format!("{base}/v2"));
    TidalApi::new(Arc::new(client), "1", "US")
}

async fn only_gets(server: &MockServer) {
    let requests = server.received_requests().await.unwrap();
    assert!(!requests.is_empty());
    assert!(requests.iter().all(|r| r.method == wiremock::http::Method::GET), "a write reached Tidal");
}

fn want(title: &str, artist: &str) -> SongCandidate {
    SongCandidate { title: title.into(), artist: artist.into(), tag: EntryTag::Always, note: "note".into() }
}

fn empty_search() -> serde_json::Value {
    let page = json!({"limit": 0, "offset": 0, "totalNumberOfItems": 0, "items": []});
    json!({"artists": page, "albums": page, "playlists": page, "tracks": page, "videos": page})
}

// ---- store ----

fn playlist(id: &str, updated_ms: u64) -> HomePlaylist {
    HomePlaylist { id: id.into(), name: format!("List {id}"), updated_ms, created_ms: 1, ..HomePlaylist::default() }
}

#[test]
fn store_saves_lists_loads_and_removes() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlaylistStore::in_app_dir(dir.path());
    assert!(store.list().is_empty(), "no folder gives an empty list");

    store.save(&playlist("old", 10)).unwrap();
    store.save(&playlist("new", 30)).unwrap();
    store.save(&playlist("mid", 20)).unwrap();
    let ids: Vec<_> = store.list().into_iter().map(|p| p.id).collect();
    assert_eq!(ids, ["new", "mid", "old"]);
    assert_eq!(store.load("mid").unwrap().name, "List mid");

    let file = store.file_path("mid").unwrap();
    assert!(file.starts_with(dir.path().join("playlists")));
    assert!(store.remove("mid").unwrap());
    assert!(!store.remove("mid").unwrap());
    assert!(store.file_path("mid").is_none());
    assert!(store.load("mid").is_err());
}

#[test]
fn store_skips_but_keeps_files_it_cannot_read() {
    let dir = tempfile::tempdir().unwrap();
    let store = PlaylistStore::new(dir.path());
    store.save(&playlist("good", 1)).unwrap();
    std::fs::write(dir.path().join("broken.json"), b"{ not json").unwrap();
    std::fs::write(dir.path().join(".homefi-x.tmp"), serde_json::to_vec(&playlist("tmp", 2)).unwrap()).unwrap();
    std::fs::write(dir.path().join("notes.txt"), b"hello").unwrap();

    let ids: Vec<_> = store.list().into_iter().map(|p| p.id).collect();
    assert_eq!(ids, ["good"]);
    assert!(dir.path().join("broken.json").exists(), "an unreadable file is never deleted");
}

#[test]
fn store_ignores_unknown_fields() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("p1.json"), br#"{"id":"p1","name":"X","from_the_future":{"a":1}}"#).unwrap();
    let store = PlaylistStore::new(dir.path());
    assert_eq!(store.load("p1").unwrap().name, "X");
    assert_eq!(store.list().len(), 1);
}

#[test]
fn store_rejects_ids_that_leave_the_folder() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("playlists");
    std::fs::create_dir_all(&dir).unwrap();
    let outside = root.path().join("evil.json");
    std::fs::write(&outside, serde_json::to_vec(&playlist("evil", 1)).unwrap()).unwrap();
    let store = PlaylistStore::new(&dir);

    for bad in ["../evil", "", "a/b", "a\\b", "..", "x.json", &"a".repeat(65)] {
        assert!(store.load(bad).is_err(), "load {bad:?}");
        assert!(!store.remove(bad).unwrap(), "remove {bad:?}");
        assert!(store.file_path(bad).is_none(), "file_path {bad:?}");
        assert!(store.save(&playlist(bad, 1)).is_err(), "save {bad:?}");
    }
    assert!(outside.exists(), "nothing outside the folder is touched");
}

// ---- resolver ----

async fn mount_search(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path("/v1/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

fn fixture() -> serde_json::Value {
    serde_json::from_str(SEARCH_FIXTURE).unwrap()
}

#[tokio::test]
async fn the_studio_track_wins() {
    let server = MockServer::start().await;
    mount_search(&server, fixture()).await;
    let api = api(&server);
    let library = LibraryIndex::default();
    let resolver = Resolver { api: Some(&api), library: &library, prefer_library: true };

    let out = resolver.resolve_all(vec![want("Freak On A Leash", "Korn")], &|_, _| {}).await;
    let entry = out[0].entry.as_ref().expect("matched");
    assert_eq!(entry.tidal_id.as_deref(), Some("103"));
    assert_eq!((entry.artist.as_str(), entry.album.as_str()), ("Korn", "Follow the Leader"));
    assert_eq!((entry.tag, entry.note.as_str()), (EntryTag::Always, "note"));
    assert_eq!(entry.duration_ms, 184_000);
    assert!(out[0].confidence >= MIN_CONFIDENCE);
    only_gets(&server).await;
}

#[tokio::test]
async fn an_unknown_song_is_not_found() {
    let server = MockServer::start().await;
    mount_search(&server, empty_search()).await;
    let api = api(&server);
    let library = LibraryIndex::default();
    let resolver = Resolver { api: Some(&api), library: &library, prefer_library: true };

    let out = resolver.resolve_all(vec![want("A Song Nobody Wrote", "Korn")], &|_, _| {}).await;
    assert!(out[0].entry.is_none());
    only_gets(&server).await;
}

#[tokio::test]
async fn a_song_the_search_only_hints_at_is_not_found() {
    let server = MockServer::start().await;
    mount_search(&server, fixture()).await;
    let api = api(&server);
    let library = LibraryIndex::default();
    let resolver = Resolver { api: Some(&api), library: &library, prefer_library: true };
    // the fixture has no such song: the other artist's hit must not be accepted
    let out = resolver.resolve_all(vec![want("Dreaming of Orange Skies", "Korn")], &|_, _| {}).await;
    assert!(out[0].entry.is_none());
}

fn library_with_studio_isrc() -> LibraryIndex {
    LibraryIndex::new(vec![LibraryRef {
        path: "/music/Korn/Freak.flac".into(),
        title: "Freak On A Leash".into(),
        artist: "Korn".into(),
        album: "Follow the Leader".into(),
        isrc: Some("ussm19800100".into()),
        duration_ms: 255_000,
        ..LibraryRef::default()
    }])
}

#[tokio::test]
async fn library_copies_are_used_when_preferred() {
    let server = MockServer::start().await;
    mount_search(&server, fixture()).await;
    let api = api(&server);
    let library = library_with_studio_isrc();

    let on = Resolver { api: Some(&api), library: &library, prefer_library: true };
    let entry = on.resolve_all(vec![want("Freak On A Leash", "Korn")], &|_, _| {}).await.remove(0).entry.unwrap();
    assert_eq!(entry.local_path.as_deref(), Some("/music/Korn/Freak.flac"));
    assert_eq!(entry.tidal_id.as_deref(), Some("103"));

    let off = Resolver { api: Some(&api), library: &library, prefer_library: false };
    let entry = off.resolve_all(vec![want("Freak On A Leash", "Korn")], &|_, _| {}).await.remove(0).entry.unwrap();
    assert!(entry.local_path.is_none());
    assert_eq!(entry.tidal_id.as_deref(), Some("103"));
}

#[tokio::test]
async fn without_tidal_only_library_songs_resolve() {
    let library = library_with_studio_isrc();
    let resolver = Resolver { api: None, library: &library, prefer_library: false };
    let out = resolver.resolve_all(vec![want("Freak On A Leash", "Korn"), want("Blind", "Korn")], &|_, _| {}).await;
    let entry = out[0].entry.as_ref().unwrap();
    assert_eq!(entry.local_path.as_deref(), Some("/music/Korn/Freak.flac"));
    assert!(entry.tidal_id.is_none());
    assert!(out[1].entry.is_none());
}

#[tokio::test]
async fn a_failing_search_leaves_the_song_unmatched() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/v1/search")).respond_with(ResponseTemplate::new(500)).mount(&server).await;
    let api = api(&server);
    let library = LibraryIndex::default();
    let resolver = Resolver { api: Some(&api), library: &library, prefer_library: true };

    let out = resolver.resolve_all(vec![want("Freak On A Leash", "Korn")], &|_, _| {}).await;
    assert_eq!(out.len(), 1);
    assert!(out[0].entry.is_none());
    only_gets(&server).await;
}

#[tokio::test]
async fn results_keep_the_order_and_progress_reaches_the_end() {
    let server = MockServer::start().await;
    mount_search(&server, fixture()).await;
    let api = api(&server);
    let library = LibraryIndex::default();
    let resolver = Resolver { api: Some(&api), library: &library, prefer_library: true };

    let titles = ["Freak On A Leash", "Nobody Wrote This", "Freak On A Leash", "Another Invention", "Freak On A Leash"];
    let seen = Mutex::new(Vec::new());
    let out = resolver
        .resolve_all(titles.iter().map(|t| want(t, "Korn")).collect(), &|done, total| {
            seen.lock().unwrap().push((done, total))
        })
        .await;

    let got: Vec<_> = out.iter().map(|r| r.candidate.title.as_str()).collect();
    assert_eq!(got, titles);
    let matched: Vec<_> = out.iter().map(|r| r.entry.is_some()).collect();
    assert_eq!(matched, [true, false, true, false, true]);
    let seen = seen.into_inner().unwrap();
    assert_eq!(seen.len(), titles.len());
    assert_eq!(seen.last(), Some(&(titles.len(), titles.len())));
    assert!(seen.iter().all(|&(_, total)| total == titles.len()));
}

// ---- most played ----

fn artist(id: i64, name: &str) -> serde_json::Value {
    json!({"id": id, "name": name, "type": "MAIN"})
}

fn top_track(id: i64) -> serde_json::Value {
    json!({
        "id": id, "title": format!("Song {id}"), "duration": 200, "allowStreaming": true,
        "artist": artist(7, "Korn"), "artists": [artist(7, "Korn")], "album": {"id": 1, "title": "A"}
    })
}

async fn mount_artists(server: &MockServer, artists: Vec<serde_json::Value>) {
    let mut body = empty_search();
    body["artists"] = json!({"limit": 5, "offset": 0, "totalNumberOfItems": artists.len(), "items": artists});
    mount_search(server, body).await;
}

#[tokio::test]
async fn most_played_picks_the_exact_artist_and_caps_the_list() {
    let server = MockServer::start().await;
    mount_artists(&server, vec![artist(5, "Korn Tribute Band"), artist(7, "Korn")]).await;
    Mock::given(method("GET"))
        .and(path_regex(r"^/v1/artists/7/toptracks$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "limit": 100, "offset": 0, "totalNumberOfItems": 5,
            "items": (1..=5).map(top_track).collect::<Vec<_>>()
        })))
        .mount(&server)
        .await;
    let api = api(&server);

    let (name, tracks) = most_played(&api, "korn", 3).await.unwrap().unwrap();
    assert_eq!(name, "Korn");
    assert_eq!(tracks.iter().map(|t| t.id).collect::<Vec<_>>(), [1, 2, 3]);
    only_gets(&server).await;
}

#[tokio::test]
async fn most_played_falls_back_to_the_first_artist() {
    let server = MockServer::start().await;
    mount_artists(&server, vec![artist(9, "Slipknot")]).await;
    Mock::given(method("GET"))
        .and(path("/v1/artists/9/toptracks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "limit": 100, "offset": 0, "totalNumberOfItems": 1, "items": [top_track(1)]
        })))
        .mount(&server)
        .await;
    let (name, tracks) = most_played(&api(&server), "Slipknott", 25).await.unwrap().unwrap();
    assert_eq!((name.as_str(), tracks.len()), ("Slipknot", 1));
}

#[tokio::test]
async fn most_played_without_an_artist_is_none() {
    let server = MockServer::start().await;
    mount_search(&server, empty_search()).await;
    assert!(most_played(&api(&server), "Nobody Here", 25).await.unwrap().is_none());
    only_gets(&server).await;
}
