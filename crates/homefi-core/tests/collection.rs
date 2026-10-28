//! Favorites and playlist edits against a mock Tidal server.

use std::sync::Arc;

use homefi_core::api::{TidalApi, TidalClient};
use homefi_core::resource::ResourceKind;
use wiremock::matchers::{body_string_contains, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn api(server: &MockServer) -> TidalApi {
    let base = server.uri();
    let client = TidalClient::new("token").with_base_urls(format!("{base}/v1"), format!("{base}/v2"));
    TidalApi::new(Arc::new(client), "1", "US")
}

fn playlist_json(uuid: &str) -> serde_json::Value {
    serde_json::json!({ "uuid": uuid, "title": "Road trip", "numberOfTracks": 0, "duration": 0 })
}

#[tokio::test]
async fn favorites_are_added_and_removed() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/users/1/favorites/tracks"))
        .and(body_string_contains("trackIds=42"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/users/1/favorites/albums/7"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let api = api(&server);
    api.add_favorite(ResourceKind::Track, "42").await.unwrap();
    api.remove_favorite(ResourceKind::Album, "7").await.unwrap();
    assert!(api.add_favorite(ResourceKind::Mix, "x").await.is_err());
}

#[tokio::test]
async fn playlist_is_created_and_edited_with_its_etag() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v2/my-collection/playlists/folders/create-playlist"))
        .and(query_param("name", "Road trip"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "data": { "uuid": "abc" } })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/playlists/abc"))
        .respond_with(ResponseTemplate::new(200).insert_header("ETag", "\"17\"").set_body_json(playlist_json("abc")))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/playlists/abc/items"))
        .and(header("If-None-Match", "\"17\""))
        .and(body_string_contains("trackIds=1%2C2"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/playlists/abc/items/0"))
        .and(header("If-None-Match", "\"17\""))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    let api = api(&server);
    let created = api.create_playlist("Road trip", "").await.unwrap();
    assert_eq!(created.uuid, "abc");
    api.add_to_playlist("abc", &["1".into(), "2".into()]).await.unwrap();
    api.remove_from_playlist("abc", 0).await.unwrap();
}

#[tokio::test]
async fn create_playlist_falls_back_to_v1() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v2/my-collection/playlists/folders/create-playlist"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({ "status": 404 })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/users/1/playlists"))
        .and(body_string_contains("title=Mix"))
        .respond_with(ResponseTemplate::new(201).set_body_json(playlist_json("v1")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/playlists/v1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(playlist_json("v1")))
        .mount(&server)
        .await;

    assert_eq!(api(&server).create_playlist("Mix", "").await.unwrap().uuid, "v1");
}
