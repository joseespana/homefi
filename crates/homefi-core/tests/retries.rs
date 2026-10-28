//! Busy answers from Tidal (`429`, `5xx`) are retried with backoff, honouring
//! `Retry-After`, against a mock server.

use std::sync::Arc;
use std::time::{Duration, Instant};

use homefi_core::api::{all_pages, TidalApi, TidalClient};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn api(server: &MockServer) -> TidalApi {
    let base = server.uri();
    let client = TidalClient::new("token").with_base_urls(format!("{base}/v1"), format!("{base}/v2"));
    TidalApi::new(Arc::new(client), "1", "US")
}

fn playlist_json() -> serde_json::Value {
    serde_json::json!({ "uuid": "abc", "title": "Road trip", "numberOfTracks": 0, "duration": 0 })
}

fn busy(retry_after: &str) -> ResponseTemplate {
    ResponseTemplate::new(429)
        .insert_header("Retry-After", retry_after)
        .set_body_json(serde_json::json!({ "status": 429, "subStatus": 0, "userMessage": "Too many requests" }))
}

#[tokio::test]
async fn a_429_waits_for_retry_after_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/playlists/abc"))
        .respond_with(busy("1"))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/playlists/abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(playlist_json()))
        .expect(1)
        .mount(&server)
        .await;

    let started = Instant::now();
    let playlist = api(&server).playlist("abc").await.unwrap();
    assert_eq!(playlist.title, "Road trip");
    assert!(started.elapsed() >= Duration::from_secs(1), "Retry-After was honoured");
}

#[tokio::test]
async fn server_errors_give_up_after_three_retries() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/playlists/abc"))
        .respond_with(
            ResponseTemplate::new(503)
                .insert_header("Retry-After", "0")
                .set_body_json(serde_json::json!({ "status": 503, "userMessage": "Down" })),
        )
        .expect(4)
        .mount(&server)
        .await;
    // a 404 is an answer, not a hiccup: asked once
    Mock::given(method("GET"))
        .and(path("/v1/playlists/gone"))
        .respond_with(
            ResponseTemplate::new(404).set_body_json(serde_json::json!({ "status": 404, "userMessage": "Nope" })),
        )
        .expect(1)
        .mount(&server)
        .await;

    let api = api(&server);
    let err = api.playlist("abc").await.unwrap_err();
    assert!(err.is_retryable(), "{err}");
    assert!(err.to_string().contains("503"), "{err}");
    assert!(!api.playlist("gone").await.unwrap_err().is_retryable());
}

fn page(offset: i64, ids: &[i64]) -> serde_json::Value {
    let items: Vec<_> = ids
        .iter()
        .map(|id| {
            serde_json::json!({ "type": "track", "item": {
                "id": id, "title": format!("T{id}"), "artists": [], "album": { "id": 9, "title": "X" }
            }})
        })
        .collect();
    serde_json::json!({ "limit": 2, "offset": offset, "totalNumberOfItems": 6, "items": items })
}

#[tokio::test]
async fn one_busy_page_does_not_lose_the_whole_list() {
    let server = MockServer::start().await;
    let items = "/v1/playlists/abc/items";
    // the page at offset 2 stays busy through the client's own retries (1 + 3)...
    Mock::given(method("GET"))
        .and(path(items))
        .and(query_param("offset", "2"))
        .respond_with(busy("0"))
        .up_to_n_times(4)
        .mount(&server)
        .await;
    // ...and answers when the list asks for it again
    for (offset, ids) in [(0, [1, 2]), (2, [3, 4]), (4, [5, 6])] {
        Mock::given(method("GET"))
            .and(path(items))
            .and(query_param("offset", offset.to_string()))
            .respond_with(ResponseTemplate::new(200).set_body_json(page(offset, &ids)))
            .mount(&server)
            .await;
    }

    let api = api(&server);
    let list = all_pages(|o| api.playlist_items("abc", o)).await.unwrap();
    let ids: Vec<i64> = list.iter().map(|l| l.item.id()).collect();
    assert_eq!(ids, vec![1, 2, 3, 4, 5, 6]);
}
