//! "A new version is available": the GitHub releases feed against a mock server.

use homefi_core::update::{sha256_of, ReleaseFeed, ANDROID_APK, REPO, SHA256SUMS};
use wiremock::matchers::{header_exists, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn release(server: &MockServer, tag: &str, prerelease: bool) -> serde_json::Value {
    let base = server.uri();
    serde_json::json!({
        "tag_name": tag,
        "name": format!("HomeFi {}", tag.trim_start_matches('v')),
        "body": "### Added\n\n- Auto-update\n\n---\n**Personal, non-commercial use only.**",
        "prerelease": prerelease,
        "draft": false,
        "html_url": format!("https://github.com/{REPO}/releases/tag/{tag}"),
        "published_at": "2026-10-28T10:00:00Z",
        "assets": [
            { "name": ANDROID_APK, "browser_download_url": format!("{base}/dl/{tag}/{ANDROID_APK}"), "size": 1234 },
            { "name": SHA256SUMS, "browser_download_url": format!("{base}/dl/{tag}/{SHA256SUMS}"), "size": 99 },
        ],
        // fields the feed doesn't use are ignored
        "author": { "login": "someone" },
    })
}

#[tokio::test]
async fn the_stable_channel_reads_releases_latest() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/releases/latest")))
        .and(header_exists("user-agent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(release(&server, "v1.2.0", false)))
        .expect(3)
        .mount(&server)
        .await;
    let feed = ReleaseFeed::with_base("HomeFi-test", &server.uri());

    let update = feed.update_for("1.1.0", false).await.unwrap().expect("1.2.0 is newer");
    assert_eq!(update.version_string(), "1.2.0");
    assert_eq!(update.notes(), "### Added\n\n- Auto-update");
    assert!(update.asset(ANDROID_APK).unwrap().browser_download_url.ends_with("/dl/v1.2.0/HomeFi-android.apk"));
    // the same version, or a newer one installed, is no update
    assert!(feed.update_for("1.2.0", false).await.unwrap().is_none());
    assert!(feed.update_for("1.3.0-beta.1", false).await.unwrap().is_none());
}

#[tokio::test]
async fn no_release_yet_is_no_update() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/releases/latest")))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({ "message": "Not Found" })))
        .mount(&server)
        .await;
    let feed = ReleaseFeed::with_base("HomeFi-test", &server.uri());
    assert!(feed.newest(false).await.unwrap().is_none());
}

#[tokio::test]
async fn the_prerelease_channel_picks_the_newest_of_the_list() {
    let server = MockServer::start().await;
    let list = serde_json::json!([
        release(&server, "v1.2.0", false),
        release(&server, "v1.3.0-beta.2", true),
        release(&server, "v1.3.0-beta.1", true),
        release(&server, "v1.1.0", false),
    ]);
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/releases")))
        .and(query_param("per_page", "30"))
        .respond_with(ResponseTemplate::new(200).set_body_json(list))
        .mount(&server)
        .await;
    let feed = ReleaseFeed::with_base("HomeFi-test", &server.uri());
    let update = feed.update_for("1.2.0", true).await.unwrap().expect("the beta is newer");
    assert_eq!(update.tag_name, "v1.3.0-beta.2");
    assert!(update.prerelease);
    assert!(feed.update_for("1.3.0", true).await.unwrap().is_none());
}

#[tokio::test]
async fn errors_and_garbage_are_errors_not_updates() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/releases/latest")))
        .respond_with(ResponseTemplate::new(403).set_body_string("rate limited"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{REPO}/releases")))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>"))
        .mount(&server)
        .await;
    let feed = ReleaseFeed::with_base("HomeFi-test", &server.uri());
    assert!(feed.newest(false).await.is_err());
    assert!(feed.newest(true).await.is_err());
}

#[tokio::test]
async fn checksums_come_from_the_release() {
    let server = MockServer::start().await;
    let hash = "0123456789abcdef".repeat(4);
    Mock::given(method("GET"))
        .and(path("/dl/v1.2.0/SHA256SUMS"))
        .respond_with(ResponseTemplate::new(200).set_body_string(format!("{hash}  {ANDROID_APK}\n")))
        .mount(&server)
        .await;
    let feed = ReleaseFeed::with_base("HomeFi-test", &server.uri());
    let sums = feed.text(&format!("{}/dl/v1.2.0/SHA256SUMS", server.uri())).await.unwrap();
    assert_eq!(sha256_of(&sums, ANDROID_APK), Some(hash));
}
