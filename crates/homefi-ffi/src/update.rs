//! "HomeFi X is available" for the Android app: the newest release on GitHub
//! (the only call is to GitHub's public API; nothing about the user is sent).
//! Downloading and installing the APK is the app's job.

use homefi_core::update::{self, Release, ReleaseFeed, ANDROID_APK, SHA256SUMS};

use crate::Result;

/// A newer HomeFi release.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AppRelease {
    /// `1.2.0`
    pub version: String,
    /// `v1.2.0`
    pub tag: String,
    /// The CHANGELOG section (Markdown), without the release page's legal notice.
    pub notes: String,
    /// The release page, for "What's new" in the browser.
    pub page_url: String,
    pub prerelease: bool,
    /// ISO 8601, when GitHub says.
    pub published_at: Option<String>,
    /// `HomeFi-android.apk` of this release, when it has one.
    pub apk_url: Option<String>,
    pub apk_size: u64,
    /// The release's `SHA256SUMS`, to check the APK before installing it.
    pub sha256sums_url: Option<String>,
}

impl From<Release> for AppRelease {
    fn from(r: Release) -> Self {
        let apk = r.asset(ANDROID_APK).cloned();
        let sums = r.asset(SHA256SUMS).map(|a| a.browser_download_url.clone());
        Self {
            version: r.version_string(),
            notes: r.notes(),
            tag: r.tag_name,
            page_url: r.html_url,
            prerelease: r.prerelease,
            published_at: r.published_at,
            apk_size: apk.as_ref().map_or(0, |a| a.size),
            apk_url: apk.map(|a| a.browser_download_url),
            sha256sums_url: sums,
        }
    }
}

/// The newest release when it is newer than `current_version` (the app's
/// `versionName`); pre-releases only with `include_prereleases`.
#[uniffi::export]
pub async fn check_for_update(current_version: String, include_prereleases: bool) -> Result<Option<AppRelease>> {
    crate::runtime::run(async move {
        let feed = ReleaseFeed::new("HomeFi-android");
        Ok(feed.update_for(&current_version, include_prereleases).await?.map(AppRelease::from))
    })
    .await
}

/// The SHA-256 (lower-case hex) listed for `file_name` in the `SHA256SUMS` at
/// `sums_url`; `None` when the file isn't listed.
#[uniffi::export]
pub async fn release_sha256(sums_url: String, file_name: String) -> Result<Option<String>> {
    crate::runtime::run(async move {
        let sums = ReleaseFeed::new("HomeFi-android").text(&sums_url).await?;
        Ok(update::sha256_of(&sums, &file_name))
    })
    .await
}

/// `true` when `candidate` is a newer version than `current` (semver; garbled
/// versions are never newer).
#[uniffi::export]
pub fn is_newer_version(candidate: String, current: String) -> bool {
    update::is_newer(&candidate, &current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use homefi_core::update::Asset;

    #[test]
    fn a_release_maps_to_the_apk_and_its_checksums() {
        let asset =
            |name: &str, size| Asset { name: name.into(), browser_download_url: format!("https://x/{name}"), size };
        let r = Release {
            tag_name: "v1.2.0".into(),
            name: None,
            body: Some("- New\n\n---\nnotice".into()),
            prerelease: false,
            draft: false,
            html_url: "https://github.com/joseespana/homefi/releases/tag/v1.2.0".into(),
            published_at: Some("2026-10-28T10:00:00Z".into()),
            assets: vec![asset("HomeFi-1.2.0-android.apk", 1), asset(ANDROID_APK, 42), asset(SHA256SUMS, 3)],
        };
        let a = AppRelease::from(r);
        assert_eq!(a.version, "1.2.0");
        assert_eq!(a.notes, "- New");
        assert_eq!(a.apk_url.as_deref(), Some("https://x/HomeFi-android.apk"));
        assert_eq!(a.apk_size, 42);
        assert_eq!(a.sha256sums_url.as_deref(), Some("https://x/SHA256SUMS"));
        assert!(is_newer_version("1.2.0".into(), "1.1.0".into()));
    }
}
