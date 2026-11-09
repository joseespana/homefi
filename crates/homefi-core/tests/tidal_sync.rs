//! Saving HomeFi playlists to the TIDAL account, against a mock Tidal server only.

use std::sync::Arc;

use homefi_core::api::{TidalApi, TidalClient};
use homefi_core::playlist::{
    delete_from_tidal, save_stored_to_tidal, save_to_tidal, HomePlaylist, PlaylistEntry, PlaylistStore,
};
use serde_json::json;
use wiremock::matchers::{body_string_contains, header, method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const SEARCH_FIXTURE: &str = include_str!("fixtures/playlists/tidal_search_freak.json");

fn api(server: &MockServer) -> TidalApi {
    let base = server.uri();
    let client = TidalClient::new("token").with_base_urls(format!("{base}/v1"), format!("{base}/v2"));
    TidalApi::new(Arc::new(client), "1", "US")
}

fn on_tidal(id: &str, title: &str) -> PlaylistEntry {
    PlaylistEntry { title: title.into(), artist: "Korn".into(), tidal_id: Some(id.into()), ..PlaylistEntry::default() }
}

fn local_only(title: &str, isrc: Option<&str>) -> PlaylistEntry {
    PlaylistEntry {
        title: title.into(),
        artist: "Korn".into(),
        isrc: isrc.map(str::to_owned),
        local_path: Some(format!("/m/{title}.flac")),
        ..PlaylistEntry::default()
    }
}

fn home(entries: Vec<PlaylistEntry>, tidal_uuid: Option<&str>) -> HomePlaylist {
    HomePlaylist {
        id: "p1".into(),
        name: "Korn · Tour setlist".into(),
        description: "From 8 recent shows".into(),
        entries,
        tidal_uuid: tidal_uuid.map(str::to_owned),
        ..HomePlaylist::default()
    }
}

fn tidal_playlist(uuid: &str, title: &str, description: &str, tracks: usize) -> serde_json::Value {
    json!({ "uuid": uuid, "title": title, "description": description, "numberOfTracks": tracks, "numberOfVideos": 0 })
}

/// `GET playlists/{uuid}` with an ETag (the edits need it).
async fn mount_playlist(server: &MockServer, body: serde_json::Value) {
    let uuid = body["uuid"].as_str().unwrap().to_owned();
    Mock::given(method("GET"))
        .and(path(format!("/v1/playlists/{uuid}")))
        .respond_with(ResponseTemplate::new(200).insert_header("ETag", "\"5\"").set_body_json(body))
        .mount(server)
        .await;
}

async fn mount_create(server: &MockServer, uuid: &str, name: &str, description: &str) {
    Mock::given(method("PUT"))
        .and(path("/v2/my-collection/playlists/folders/create-playlist"))
        .and(query_param("name", name))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": { "uuid": uuid } })))
        .expect(1)
        .mount(server)
        .await;
    mount_playlist(server, tidal_playlist(uuid, name, description, 0)).await;
}

async fn requests(server: &MockServer, verb: &str) -> Vec<Request> {
    let all = server.received_requests().await.unwrap();
    all.into_iter().filter(|r| r.method.as_str() == verb).collect()
}

fn form(r: &Request) -> String {
    String::from_utf8_lossy(&r.body).into_owned()
}

#[tokio::test]
async fn a_new_playlist_is_created_with_its_songs_in_order_and_once() {
    let server = MockServer::start().await;
    mount_create(&server, "abc", "Korn · Tour setlist", "From 8 recent shows").await;
    Mock::given(method("POST"))
        .and(path("/v1/playlists/abc/items"))
        .and(header("If-None-Match", "\"5\""))
        .and(body_string_contains("trackIds=2%2C1"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    let mut playlist = home(vec![on_tidal("2", "Blind"), on_tidal("1", "Falling"), on_tidal("2", "Blind")], None);
    let report = save_to_tidal(&api(&server), &mut playlist).await.unwrap();
    assert_eq!((report.uuid.as_str(), report.created, report.saved), ("abc", true, 2));
    assert!(report.skipped.is_empty());
    assert_eq!(playlist.tidal_uuid.as_deref(), Some("abc"));
    assert!(requests(&server, "DELETE").await.is_empty(), "a new playlist has nothing to remove");
}

#[tokio::test]
async fn an_existing_copy_is_renamed_and_its_items_replaced() {
    let server = MockServer::start().await;
    mount_playlist(&server, tidal_playlist("old", "Old name", "From 8 recent shows", 3)).await;
    Mock::given(method("POST"))
        .and(path("/v1/playlists/old"))
        .and(header("If-None-Match", "\"5\""))
        .and(body_string_contains("title=Korn"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/playlists/old/items/0,1,2"))
        .and(header("If-None-Match", "\"5\""))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/playlists/old/items"))
        .and(body_string_contains("trackIds=9"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT")).respond_with(ResponseTemplate::new(500)).expect(0).mount(&server).await;

    let mut playlist = home(vec![on_tidal("9", "Blind")], Some("old"));
    let report = save_to_tidal(&api(&server), &mut playlist).await.unwrap();
    assert_eq!((report.uuid.as_str(), report.created, report.saved), ("old", false, 1));
    assert_eq!(playlist.tidal_uuid.as_deref(), Some("old"));
}

#[tokio::test]
async fn long_playlists_go_in_chunks_of_a_hundred() {
    let server = MockServer::start().await;
    let name = "Korn · Tour setlist";
    mount_playlist(&server, tidal_playlist("big", name, "From 8 recent shows", 150)).await;
    Mock::given(method("DELETE")).respond_with(ResponseTemplate::new(200)).expect(2).mount(&server).await;
    Mock::given(method("POST"))
        .and(path("/v1/playlists/big/items"))
        .respond_with(ResponseTemplate::new(200))
        .expect(2)
        .mount(&server)
        .await;

    let entries = (1..=150).map(|i| on_tidal(&i.to_string(), &format!("Song {i}"))).collect();
    let mut playlist = home(entries, Some("big"));
    let report = save_to_tidal(&api(&server), &mut playlist).await.unwrap();
    assert_eq!(report.saved, 150);

    let deletes: Vec<String> = requests(&server, "DELETE").await.iter().map(|r| r.url.path().to_owned()).collect();
    let tail = (50..150).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    let head = (0..50).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    assert_eq!(deletes, [format!("/v1/playlists/big/items/{tail}"), format!("/v1/playlists/big/items/{head}")]);
    let adds: Vec<String> = requests(&server, "POST").await.iter().map(form).collect();
    assert!(adds[0].contains("trackIds=1%2C2%2C") && adds[0].contains("%2C100&"), "first hundred: {}", adds[0]);
    assert!(adds[1].contains("trackIds=101%2C") && adds[1].contains("%2C150&"), "the rest: {}", adds[1]);
}

#[tokio::test]
async fn a_copy_deleted_on_tidal_is_made_again() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/playlists/gone"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_json(json!({ "status": 404, "subStatus": 2001, "userMessage": "Not found" })),
        )
        .mount(&server)
        .await;
    mount_create(&server, "fresh", "Korn · Tour setlist", "From 8 recent shows").await;
    Mock::given(method("POST"))
        .and(path("/v1/playlists/fresh/items"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    let mut playlist = home(vec![on_tidal("9", "Blind")], Some("gone"));
    let report = save_to_tidal(&api(&server), &mut playlist).await.unwrap();
    assert!(report.created);
    assert_eq!(playlist.tidal_uuid.as_deref(), Some("fresh"));
}

#[tokio::test]
async fn library_songs_are_found_by_isrc_then_by_name_and_the_rest_is_reported() {
    let server = MockServer::start().await;
    mount_create(&server, "abc", "Korn · Tour setlist", "From 8 recent shows").await;
    Mock::given(method("GET"))
        .and(path("/v1/tracks"))
        .and(query_param("isrc", "USSM19400123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "limit": 100, "offset": 0, "totalNumberOfItems": 2,
            "items": [
                { "id": 76, "title": "Blind", "duration": 259, "allowStreaming": false, "album": { "id": 1 } },
                { "id": 77, "title": "Blind", "duration": 259, "allowStreaming": true, "album": { "id": 1 } }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;
    let search: serde_json::Value = serde_json::from_str(SEARCH_FIXTURE).unwrap();
    Mock::given(method("GET"))
        .and(path("/v1/search"))
        .and(query_param("query", "Korn Freak On A Leash"))
        .respond_with(ResponseTemplate::new(200).set_body_json(search))
        .mount(&server)
        .await;
    let page = json!({"limit": 0, "offset": 0, "totalNumberOfItems": 0, "items": []});
    Mock::given(method("GET"))
        .and(path("/v1/search"))
        .and(query_param("query", "Korn Dreaming of Orange Skies"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                json!({"artists": page, "albums": page, "playlists": page, "tracks": page, "videos": page}),
            ),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/playlists/abc/items"))
        .and(body_string_contains("trackIds=77%2C103&"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    let mut playlist = home(
        vec![
            local_only("Blind", Some("USSM19400123")),
            local_only("Freak On A Leash", None),
            local_only("Dreaming of Orange Skies", None),
        ],
        None,
    );
    let report = save_to_tidal(&api(&server), &mut playlist).await.unwrap();
    assert_eq!(report.saved, 2);
    assert_eq!(report.skipped, ["Dreaming of Orange Skies · Korn"]);
    assert!(playlist.entries.iter().all(|e| e.tidal_id.is_none()), "the entries themselves are not changed");
}

#[tokio::test]
async fn the_stored_playlist_remembers_its_copy_and_a_stale_save_keeps_it() {
    let server = MockServer::start().await;
    mount_create(&server, "abc", "Korn · Tour setlist", "From 8 recent shows").await;
    Mock::given(method("POST"))
        .and(path("/v1/playlists/abc/items"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let store = PlaylistStore::in_app_dir(dir.path());
    let screen_copy = home(vec![on_tidal("9", "Blind")], None);
    store.save(&screen_copy).unwrap();

    let (stored, report) = save_stored_to_tidal(&api(&server), &store, "p1").await.unwrap();
    assert_eq!((stored.tidal_uuid.as_deref(), report.created), (Some("abc"), true));
    assert_eq!(store.load("p1").unwrap().tidal_uuid.as_deref(), Some("abc"));

    // the screen still holds the copy from before: saving it must not forget the TIDAL twin
    store.save(&HomePlaylist { name: "Renamed".into(), ..screen_copy }).unwrap();
    let reloaded = store.load("p1").unwrap();
    assert_eq!((reloaded.name.as_str(), reloaded.tidal_uuid.as_deref()), ("Renamed", Some("abc")));
    assert!(!store.set_tidal_uuid("missing", Some("x".into())).unwrap());
}

#[tokio::test]
async fn the_uuid_is_kept_even_when_adding_the_songs_fails() {
    let server = MockServer::start().await;
    mount_create(&server, "abc", "Korn · Tour setlist", "From 8 recent shows").await;
    Mock::given(method("POST"))
        .and(path("/v1/playlists/abc/items"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({ "status": 403, "userMessage": "Not allowed" })))
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let store = PlaylistStore::in_app_dir(dir.path());
    store.save(&home(vec![on_tidal("9", "Blind")], None)).unwrap();
    assert!(save_stored_to_tidal(&api(&server), &store, "p1").await.is_err());
    assert_eq!(
        store.load("p1").unwrap().tidal_uuid.as_deref(),
        Some("abc"),
        "the next save updates it, no second copy"
    );
}

#[tokio::test]
async fn deleting_the_copy_tolerates_one_already_gone() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/v1/playlists/here"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/playlists/gone"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({ "status": 404, "userMessage": "Not found" })))
        .expect(1)
        .mount(&server)
        .await;

    let api = api(&server);
    assert!(delete_from_tidal(&api, &home(vec![], Some("here"))).await.unwrap());
    assert!(!delete_from_tidal(&api, &home(vec![], Some("gone"))).await.unwrap());
    assert!(!delete_from_tidal(&api, &home(vec![], None)).await.unwrap(), "never saved: nothing to delete");
}
