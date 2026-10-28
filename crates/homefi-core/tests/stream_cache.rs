//! Fetching single tracks for playback into a cache folder.
//! Needs ffmpeg to create the served FLAC file (skipped otherwise).

use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use base64::Engine;
use homefi_core::api::{TidalApi, TidalClient};
use homefi_core::config::Config;
use homefi_core::quality::TrackQuality;
use homefi_core::stream_cache;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn flac_bytes(dir: &Path) -> Option<Vec<u8>> {
    let file = dir.join("source.flac");
    let ok = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "anullsrc=r=44100:cl=stereo"])
        .args(["-t", "0.3"])
        .arg(&file)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    ok.then(|| std::fs::read(&file).unwrap())
}

#[tokio::test]
async fn concurrent_fetches_download_once_and_reuse_the_file() {
    let work = tempfile::tempdir().unwrap();
    let Some(flac) = flac_bytes(work.path()) else {
        eprintln!("ffmpeg not available, skipping");
        return;
    };

    let server = MockServer::start().await;
    let base = server.uri();
    Mock::given(method("GET"))
        .and(path("/v1/tracks/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 42, "title": "Somebody To Love", "duration": 1, "trackNumber": 1, "volumeNumber": 1,
            "allowStreaming": true, "audioQuality": "LOSSLESS", "mediaMetadata": {"tags": ["LOSSLESS"]},
            "artist": {"id": 7, "name": "Queen", "type": "MAIN"},
            "artists": [{"id": 7, "name": "Queen", "type": "MAIN"}],
            "album": {"id": 1, "title": "A Day At The Races", "cover": null}
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/albums/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 1, "title": "A Day At The Races", "releaseDate": "1976-12-10", "type": "ALBUM",
            "numberOfTracks": 1, "artist": {"id": 7, "name": "Queen", "type": "MAIN"}
        })))
        .mount(&server)
        .await;
    let manifest = json!({
        "mimeType": "audio/flac", "codecs": "flac", "encryptionType": "NONE", "urls": [format!("{base}/media/42.flac")]
    });
    let manifest = base64::engine::general_purpose::STANDARD.encode(manifest.to_string());
    Mock::given(method("GET"))
        .and(path("/v1/tracks/42/playbackinfopostpaywall"))
        .and(query_param("audioquality", "LOSSLESS"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "trackId": 42, "audioMode": "STEREO", "audioQuality": "LOSSLESS",
            "manifestMimeType": "application/vnd.tidal.bts", "manifest": manifest, "bitDepth": 16, "sampleRate": 44100
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/media/42.flac"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(flac))
        .expect(1)
        .mount(&server)
        .await;

    let client = TidalClient::new("token").with_base_urls(format!("{base}/v1"), format!("{base}/v2"));
    let api = TidalApi::new(Arc::new(client), "1", "US");
    let config = Config::default();
    let cache = work.path().join("stream-cache");

    let (a, b) = tokio::join!(
        stream_cache::fetch(api.clone(), &config, &cache, "42", TrackQuality::High),
        stream_cache::fetch(api.clone(), &config, &cache, "42", TrackQuality::High),
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_eq!(a, cache.join("42.flac"));
    assert_eq!(a, b);
    assert!(a.exists());

    // already cached: no new stream request (the mocks expect exactly one)
    let again = stream_cache::fetch(api.clone(), &config, &cache, "42", TrackQuality::High).await.unwrap();
    assert_eq!(again, a);

    assert!(stream_cache::fetch(api, &config, &cache, "not a track", TrackQuality::High).await.is_err());
}
