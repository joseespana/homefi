//! Runs the download engine against a mocked Tidal API: album listing,
//! stream manifest, media download, path template, tags and skipping.
//! Needs ffmpeg to create the served FLAC file (skipped otherwise).

use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};

use base64::Engine;
use homefi_core::api::{TidalApi, TidalClient};
use homefi_core::config::Config;
use homefi_core::download::{DownloadOptions, Downloader, Event};
use homefi_core::resource::Resource;
use lofty::file::TaggedFileExt;
use lofty::prelude::*;
use lofty::tag::ItemKey;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn flac_bytes(dir: &Path) -> Option<Vec<u8>> {
    let file = dir.join("source.flac");
    let ok = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "anullsrc=r=44100:cl=stereo",
            "-t",
            "0.3",
        ])
        .arg(&file)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    ok.then(|| std::fs::read(&file).unwrap())
}

fn track(id: i64, number: i64, title: &str) -> serde_json::Value {
    json!({
        "id": id, "title": title, "duration": 1, "trackNumber": number, "volumeNumber": 1,
        "allowStreaming": true, "explicit": false, "isrc": format!("TEST{id}"), "audioQuality": "LOSSLESS",
        "mediaMetadata": {"tags": ["LOSSLESS"]},
        "artist": {"id": 7, "name": "Daft Punk", "type": "MAIN"},
        "artists": [{"id": 7, "name": "Daft Punk", "type": "MAIN"}],
        "album": {"id": 1, "title": "Discovery", "cover": null}
    })
}

#[tokio::test]
async fn downloads_an_album_and_skips_it_the_second_time() {
    let work = tempfile::tempdir().unwrap();
    let Some(flac) = flac_bytes(work.path()) else {
        eprintln!("ffmpeg not available, skipping");
        return;
    };

    let server = MockServer::start().await;
    let base = server.uri();

    Mock::given(method("GET"))
        .and(path("/v1/albums/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 1, "title": "Discovery", "releaseDate": "2001-03-13", "type": "ALBUM",
            "numberOfTracks": 2, "mediaMetadata": {"tags": ["LOSSLESS"]},
            "artist": {"id": 7, "name": "Daft Punk", "type": "MAIN"},
            "artists": [{"id": 7, "name": "Daft Punk", "type": "MAIN"}]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/v1/albums/1/items/credits"))
        .and(query_param("offset", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "limit": 20, "offset": 0, "totalNumberOfItems": 2,
            "items": [
                {"type": "track", "item": track(11, 1, "One More Time"),
                 "credits": [{"type": "Producer", "contributors": [{"name": "Thomas Bangalter"}]}]},
                {"type": "track", "item": track(12, 2, "Aerodynamic"), "credits": []}
            ]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/v2/albums/1/relationships/genres"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "included": [{"type": "genres", "attributes": {"name": "House"}}]
        })))
        .mount(&server)
        .await;

    for id in [11, 12] {
        let manifest = json!({
            "mimeType": "audio/flac", "codecs": "flac", "encryptionType": "NONE",
            "urls": [format!("{base}/media/{id}.flac")]
        });
        let manifest = base64::engine::general_purpose::STANDARD.encode(manifest.to_string());
        Mock::given(method("GET"))
            .and(path(format!("/v1/tracks/{id}/playbackinfopostpaywall")))
            .and(query_param("audioquality", "LOSSLESS"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "trackId": id, "audioMode": "STEREO", "audioQuality": "LOSSLESS",
                "manifestMimeType": "application/vnd.tidal.bts", "manifest": manifest,
                "bitDepth": 16, "sampleRate": 44100
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("/media/{id}.flac")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(flac.clone()))
            .mount(&server)
            .await;
    }

    let client = TidalClient::new("token").with_base_urls(format!("{base}/v1"), format!("{base}/v2"));
    let api = TidalApi::new(Arc::new(client), "1", "US");

    let music = work.path().join("music");
    let mut opts = DownloadOptions::from_config(&Config::default());
    opts.download_dir = music.clone();
    opts.scan_dir = music.clone();
    opts.template_override =
        Some("{album.artist}/{album.title} ({album.date:%Y})/{item.number:02d}. {item.title}".into());

    let events: Arc<Mutex<Vec<Event>>> = Arc::default();
    let sink = events.clone();
    let observer = Arc::new(move |e: Event| sink.lock().unwrap().push(e));
    let downloader = Downloader::new(api.clone(), opts.clone(), observer);

    let resources = [Resource::parse("https://tidal.com/browse/album/1").unwrap()];
    let summary = downloader.run(&resources).await.unwrap();

    let failures: Vec<_> =
        events.lock().unwrap().iter().filter(|e| matches!(e, Event::Failed { .. })).cloned().collect();
    assert!(failures.is_empty(), "{failures:?}");
    assert_eq!(summary.downloaded, 2);

    let first = music.join("Daft Punk/Discovery (2001)/01. One More Time.flac");
    assert!(first.exists(), "missing {}", first.display());
    assert!(music.join("Daft Punk/Discovery (2001)/02. Aerodynamic.flac").exists());

    let file = lofty::read_from_path(&first).unwrap();
    let tag = file.primary_tag().unwrap();
    assert_eq!(tag.title().as_deref(), Some("One More Time"));
    assert_eq!(tag.album().as_deref(), Some("Discovery"));
    assert_eq!(tag.track(), Some(1));
    assert_eq!(tag.genre().as_deref(), Some("House"));
    assert_eq!(tag.get_string(&ItemKey::AlbumArtist), Some("Daft Punk"));
    assert_eq!(tag.get_string(&ItemKey::Producer), Some("Thomas Bangalter"));

    // second run: everything already exists
    let downloader = Downloader::new(api, opts, Arc::new(|_: Event| {}));
    let summary = downloader.run(&resources).await.unwrap();
    assert_eq!(summary.downloaded, 0);
    assert_eq!(summary.existing, 2);
}

#[tokio::test]
async fn api_errors_are_reported_not_fatal() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/tracks/5"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "status": 404, "subStatus": 2001, "userMessage": "Track not found"
        })))
        .mount(&server)
        .await;

    let base = server.uri();
    let client = TidalClient::new("token").with_base_urls(format!("{base}/v1"), format!("{base}/v2"));
    let api = TidalApi::new(Arc::new(client), "1", "US");

    let events: Arc<Mutex<Vec<Event>>> = Arc::default();
    let sink = events.clone();
    let mut opts = DownloadOptions::from_config(&Config::default());
    opts.download_dir = tempfile::tempdir().unwrap().keep();
    let downloader = Downloader::new(api, opts, Arc::new(move |e: Event| sink.lock().unwrap().push(e)));

    let summary = downloader.run(&[Resource::parse("track/5").unwrap()]).await.unwrap();
    assert_eq!(summary.failed, 1);
    let events = events.lock().unwrap();
    assert!(
        matches!(&events[0], Event::Failed { message, .. } if message == "Tidal doesn't have this anymore, or the link is wrong.")
    );
}
