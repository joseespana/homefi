//! Catalog browsing against a mock Tidal server: artist top tracks and paging.

use std::sync::Arc;

use homefi_core::api::{all_pages, pages_up_to, TidalApi, TidalClient};
use homefi_core::catalog;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn api(server: &MockServer) -> TidalApi {
    let base = server.uri();
    let client = TidalClient::new("token").with_base_urls(format!("{base}/v1"), format!("{base}/v2"));
    TidalApi::new(Arc::new(client), "1", "US")
}

fn track(id: i64, title: &str) -> serde_json::Value {
    json!({
        "id": id, "title": title, "duration": 296, "trackNumber": 1, "volumeNumber": 1,
        "allowStreaming": true, "streamReady": true, "explicit": id % 2 == 0, "isrc": "gbumc7600017",
        "audioQuality": "LOSSLESS", "audioModes": ["STEREO"],
        "mediaMetadata": {"tags": ["LOSSLESS", "HIRES_LOSSLESS"]},
        "popularity": 80, "replayGain": -9.1, "peak": 0.99,
        "artist": {"id": 8992, "name": "Queen", "type": "MAIN", "picture": "abcd-ef"},
        "artists": [{"id": 8992, "name": "Queen", "type": "MAIN", "picture": "abcd-ef"}],
        "album": {"id": 77, "title": "A Day At The Races", "cover": "1234-5678", "vibrantColor": "#ffffff"}
    })
}

#[tokio::test]
async fn artist_top_tracks_are_paged() {
    let server = MockServer::start().await;
    for (offset, ids) in [(0, [1, 2]), (2, [3, 4])] {
        Mock::given(method("GET"))
            .and(path("/v1/artists/8992/toptracks"))
            .and(query_param("countryCode", "US"))
            .and(query_param("limit", "100"))
            .and(query_param("offset", offset.to_string()))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "limit": 2, "offset": offset, "totalNumberOfItems": 5,
                "items": ids.iter().map(|&i| track(i, &format!("Song {i}"))).collect::<Vec<_>>()
            })))
            .mount(&server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path("/v1/artists/8992/toptracks"))
        .and(query_param("offset", "4"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "limit": 2, "offset": 4, "totalNumberOfItems": 5, "items": [track(5, "Song 5")]
        })))
        .mount(&server)
        .await;

    let api = api(&server);
    let first = api.artist_top_tracks("8992", 0).await.unwrap();
    assert_eq!(first.next_offset(), Some(2));
    let hit = catalog::track_hit(&first.items[0]);
    assert_eq!((hit.id.as_str(), hit.artist.as_str(), hit.album.as_str()), ("1", "Queen", "A Day At The Races"));
    assert_eq!(hit.isrc.as_deref(), Some("GBUMC7600017"));
    assert_eq!(hit.duration_ms, Some(296_000));
    assert_eq!(hit.quality.as_deref(), Some("Hi-Res"));
    assert!(hit.cover_url.unwrap().contains("1234/5678"));

    let some = pages_up_to(3, |o| api.artist_top_tracks("8992", o)).await.unwrap();
    assert_eq!(some.iter().map(|t| t.id).collect::<Vec<_>>(), [1, 2, 3]);
    let requests = server.received_requests().await.unwrap();
    assert!(requests.iter().all(|r| !r.url.query().unwrap_or_default().contains("offset=4")), "stops at 3 items");
    let all = all_pages(|o| api.artist_top_tracks("8992", o)).await.unwrap();
    assert_eq!(all.len(), 5);
}
