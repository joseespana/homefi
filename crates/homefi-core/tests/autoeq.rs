//! AutoEq client against a local server: index and profile downloads, the
//! disk cache and offline use. One ignored test talks to the real AutoEq.

use homefi_core::dsp::autoeq::{AutoEq, HeadphoneEntry};
use homefi_core::dsp::{EqBandKind, EqSource};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const INDEX: &str = "# Index\n\
    - [Sennheiser HD 650](./oratory1990/over-ear/Sennheiser%20HD%20650) by oratory1990\n\
    - [Sennheiser HD 650](./crinacle/GRAS%2043AG-7%20over-ear/Sennheiser%20HD%20650) by crinacle on GRAS 43AG-7\n\
    - [Moondrop Aria](./crinacle/711%20in-ear/Moondrop%20Aria) by crinacle on 711\n";

const HD650: &str = "Preamp: -6.1 dB\n\
    Filter 1: ON LSC Fc 105 Hz Gain 6.4 dB Q 0.70\n\
    Filter 2: ON PK Fc 8800 Hz Gain 5.1 dB Q 1.42\n\
    Filter 3: ON HSC Fc 10000 Hz Gain -2.1 dB Q 0.70\n";

const PROFILE_PATH: &str =
    "/results/oratory1990/over-ear/Sennheiser%20HD%20650/Sennheiser%20HD%20650%20ParametricEQ.txt";

async fn server() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/results/INDEX.md"))
        .respond_with(ResponseTemplate::new(200).set_body_string(INDEX))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(PROFILE_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_string(HD650))
        .expect(1)
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn searches_downloads_and_caches() {
    let dir = tempfile::tempdir().unwrap();
    let server = server().await;
    let base = format!("{}/results", server.uri());
    let autoeq = AutoEq::with_base_url(dir.path(), &base);

    let hits = autoeq.search("hd650").await.unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].source, "oratory1990");
    assert_eq!(hits[1].rig.as_deref(), Some("GRAS 43AG-7"));
    // the second search uses the index in memory
    assert_eq!(autoeq.search("aria").await.unwrap()[0].kind, "in-ear");

    let p = autoeq.profile(&hits[0]).await.unwrap();
    assert_eq!(p.preamp_db, -6.1);
    assert_eq!(p.bands.len(), 3);
    assert_eq!(p.bands[0].kind, EqBandKind::LowShelf);
    assert_eq!(p.source, EqSource::AutoEq { model: "Sennheiser HD 650".into(), source: "oratory1990".into() });
    // cached: no second request (the mocks expect one call each)
    assert_eq!(autoeq.profile(&hits[0]).await.unwrap(), p);
    server.verify().await;

    // offline: a new client with the server gone still has both from disk
    drop(server);
    let offline = AutoEq::with_base_url(dir.path(), "http://127.0.0.1:9/results");
    assert_eq!(offline.search("HD 650").await.unwrap().len(), 2);
    assert_eq!(offline.profile(&hits[0]).await.unwrap(), p);
    // but a profile never opened can't come
    let aria = offline.search("aria").await.unwrap().remove(0);
    let err = offline.profile(&aria).await.unwrap_err();
    assert!(err.humane().contains("AutoEq"), "{}", err.humane());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn searches_at_once_download_the_index_once() {
    let dir = tempfile::tempdir().unwrap();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/results/INDEX.md"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(INDEX).set_delay(std::time::Duration::from_millis(200)),
        )
        .expect(1)
        .mount(&server)
        .await;
    let autoeq = std::sync::Arc::new(AutoEq::with_base_url(dir.path(), &format!("{}/results", server.uri())));
    let searches: Vec<_> = ["hd650", "aria", "sennheiser", "moondrop"]
        .into_iter()
        .map(|q| {
            let autoeq = autoeq.clone();
            tokio::spawn(async move { autoeq.search(q).await.unwrap().len() })
        })
        .collect();
    for s in searches {
        assert!(s.await.unwrap() > 0);
    }
    server.verify().await;
}

#[tokio::test]
async fn offline_without_cache_says_so() {
    homefi_core::humane::set_language("en");
    let dir = tempfile::tempdir().unwrap();
    let autoeq = AutoEq::with_base_url(dir.path(), "http://127.0.0.1:9/results");
    let err = autoeq.search("hd650").await.unwrap_err();
    let text = err.humane();
    assert!(text.contains("Check your connection"), "{text}");
    assert!(!text.contains("127.0.0.1"));
}

#[tokio::test]
async fn missing_profile_is_explained() {
    homefi_core::humane::set_language("en");
    let dir = tempfile::tempdir().unwrap();
    let server = MockServer::start().await;
    let autoeq = AutoEq::with_base_url(dir.path(), &format!("{}/results", server.uri()));
    let gone = HeadphoneEntry {
        name: "Gone".into(),
        source: "oratory1990".into(),
        kind: "over-ear".into(),
        rig: None,
        path: "oratory1990/over-ear/Gone".into(),
    };
    let text = autoeq.profile(&gone).await.unwrap_err().humane();
    assert!(text.contains("doesn't have this profile"), "{text}");
    // paths outside results/ are refused before any request
    let evil = HeadphoneEntry { path: "../../x".into(), ..gone };
    assert!(autoeq.profile(&evil).await.is_err());
}

/// Read-only call to the real AutoEq repository: `cargo test -p homefi-core --test autoeq -- --ignored`.
#[tokio::test]
#[ignore = "network"]
async fn live_autoeq() {
    let dir = tempfile::tempdir().unwrap();
    let autoeq = AutoEq::new(dir.path());
    let hits = autoeq.search("Sennheiser HD 650").await.unwrap();
    assert!(!hits.is_empty());
    let p = autoeq.profile(&hits[0]).await.unwrap();
    assert!(!p.bands.is_empty() && p.preamp_db < 0.0, "{p:?}");
}
