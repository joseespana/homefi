//! "A new version is available": the newest HomeFi release on GitHub.
//!
//! Used by the CLI's once-a-day notice, the desktop app's pre-release channel
//! and the Android updater. The only calls are to GitHub's public API and the
//! release's own assets: no account, no identifiers, nothing about the user
//! (GitHub sees the request like any web page visit: the IP address and a
//! `HomeFi/<version>` user agent).

use std::cmp::Ordering;
use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// `owner/name` of the repository the releases come from.
pub const REPO: &str = "joseespana/homefi";
/// GitHub's REST API.
pub const API_BASE: &str = "https://api.github.com";
/// The releases page, for "Download" when an update can't be installed in place.
pub const RELEASES_PAGE: &str = "https://github.com/joseespana/homefi/releases";
/// The apps look for a new version at most this often (plus once on launch).
pub const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 3600);
/// The CLI checks at most once a day.
pub const CLI_CHECK_INTERVAL: Duration = Duration::from_secs(24 * 3600);
/// Set to anything but `0` / empty to turn the CLI's check off.
pub const NO_UPDATE_CHECK_ENV: &str = "HOMEFI_NO_UPDATE_CHECK";
/// Name of the checksum list attached to every release.
pub const SHA256SUMS: &str = "SHA256SUMS";
/// The Android APK of a release, under its version-free name.
pub const ANDROID_APK: &str = "HomeFi-android.apk";
/// The desktop updater manifest (Tauri's `latest.json`) attached to every release.
pub const DESKTOP_MANIFEST: &str = "latest.json";

/// A semantic version (`1.2.0`, `v1.3.0-beta.2`); build metadata (`+abc`) is ignored.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    /// Pre-release identifiers (`beta`, `2`); empty for a stable release.
    pub pre: Vec<PreId>,
}

/// One dot-separated part of a pre-release (`beta` or `2`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PreId {
    Num(u64),
    Alpha(String),
}

impl Version {
    pub fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }
}

impl FromStr for Version {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let bad = || format!("`{s}` is not a version (expected x.y.z)");
        let t = s.trim();
        let t = t.strip_prefix(['v', 'V']).unwrap_or(t);
        let t = t.split_once('+').map_or(t, |(v, _)| v);
        let (core, pre) = match t.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (t, None),
        };
        let mut nums = core.split('.').map(|n| n.parse::<u64>().map_err(|_| bad()));
        let major = nums.next().ok_or_else(bad)??;
        let minor = nums.next().ok_or_else(bad)??;
        let patch = nums.next().ok_or_else(bad)??;
        if nums.next().is_some() {
            return Err(bad());
        }
        let pre = match pre {
            None => Vec::new(),
            Some("") => return Err(bad()),
            Some(p) => p
                .split('.')
                .map(|id| {
                    if id.is_empty() {
                        Err(bad())
                    } else if id.bytes().all(|b| b.is_ascii_digit()) {
                        id.parse().map(PreId::Num).map_err(|_| bad())
                    } else {
                        Ok(PreId::Alpha(id.to_owned()))
                    }
                })
                .collect::<std::result::Result<_, _>>()?,
        };
        Ok(Self { major, minor, patch, pre })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        for (i, id) in self.pre.iter().enumerate() {
            f.write_str(if i == 0 { "-" } else { "." })?;
            match id {
                PreId::Num(n) => write!(f, "{n}")?,
                PreId::Alpha(a) => f.write_str(a)?,
            }
        }
        Ok(())
    }
}

impl Ord for PreId {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (PreId::Num(a), PreId::Num(b)) => a.cmp(b),
            // numeric identifiers sort before alphanumeric ones
            (PreId::Num(_), PreId::Alpha(_)) => Ordering::Less,
            (PreId::Alpha(_), PreId::Num(_)) => Ordering::Greater,
            (PreId::Alpha(a), PreId::Alpha(b)) => a.cmp(b),
        }
    }
}

impl PartialOrd for PreId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    /// Semver precedence: `1.2.0-beta.1 < 1.2.0-beta.2 < 1.2.0 < 1.2.1`.
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch)).then_with(|| {
            match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                // a stable release is newer than its own pre-releases
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => self.pre.cmp(&other.pre),
            }
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// `true` when `candidate` is a newer version than `current`. Anything that
/// doesn't parse is never newer (no update offered on a garbled tag).
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (candidate.parse::<Version>(), current.parse::<Version>()) {
        (Ok(c), Ok(cur)) => c > cur,
        _ => false,
    }
}

/// A file attached to a release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub size: u64,
}

/// A GitHub release, as `/repos/{owner}/{repo}/releases` lists it (the fields used here).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub name: Option<String>,
    /// Release notes (Markdown).
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub draft: bool,
    pub html_url: String,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

impl Release {
    pub fn version(&self) -> Option<Version> {
        self.tag_name.parse().ok()
    }

    /// The version without the `v` of the tag (`1.2.0`).
    pub fn version_string(&self) -> String {
        self.version().map_or_else(|| self.tag_name.clone(), |v| v.to_string())
    }

    pub fn asset(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.name == name)
    }

    /// The notes for the "What's new" panel: the CHANGELOG section, without the
    /// legal notice the release page carries under a `---` rule.
    pub fn notes(&self) -> String {
        release_notes(self.body.as_deref().unwrap_or_default())
    }
}

/// Release notes without the trailing notice (everything from the last `---`
/// line on), trimmed.
pub fn release_notes(body: &str) -> String {
    let body = body.replace("\r\n", "\n");
    let cut = body.rfind("\n---\n").or_else(|| body.strip_prefix("---\n").map(|_| 0));
    match cut {
        Some(i) => body[..i].trim().to_owned(),
        None => body.trim().to_owned(),
    }
}

/// The newest release of `releases` (drafts and unparsable tags skipped;
/// pre-releases only with `include_prereleases`).
pub fn newest(releases: &[Release], include_prereleases: bool) -> Option<&Release> {
    releases
        .iter()
        .filter(|r| !r.draft && (include_prereleases || !r.prerelease))
        .filter_map(|r| {
            let v = r.version()?;
            (include_prereleases || !v.is_prerelease()).then_some((v, r))
        })
        .max_by(|(a, _), (b, _)| a.cmp(b))
        .map(|(_, r)| r)
}

/// The SHA-256 (lower-case hex) of `file` in a `SHA256SUMS` list
/// (`<hash>  <name>` or `<hash> *<name>` lines, as `sha256sum` writes them).
pub fn sha256_of(sums: &str, file: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, name) = line.trim().split_once(char::is_whitespace)?;
        let name = name.trim_start();
        let name = name.strip_prefix('*').unwrap_or(name);
        (name == file && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| hash.to_ascii_lowercase())
    })
}

/// Reads HomeFi's releases from GitHub's public API.
#[derive(Clone)]
pub struct ReleaseFeed {
    client: reqwest::Client,
    base: String,
}

impl ReleaseFeed {
    /// The real GitHub API. `app` goes in the user agent GitHub requires
    /// (`HomeFi-cli`, `HomeFi-android`...), next to the version.
    pub fn new(app: &str) -> Self {
        Self::with_base(app, API_BASE)
    }

    /// Another API root (tests, a local mirror).
    pub fn with_base(app: &str, base: &str) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(format!("{app}/{}", crate::VERSION))
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(8))
            .build()
            .unwrap_or_default();
        Self { client, base: base.trim_end_matches('/').to_owned() }
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<Option<T>> {
        let response = self.client.get(url).header("Accept", "application/vnd.github+json").send().await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let response = response.error_for_status()?;
        let text = response.text().await?;
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| Error::Decode { endpoint: url.to_owned(), message: e.to_string() })
    }

    /// The newest release: `/releases/latest` (stable only, as GitHub marks it),
    /// or with `include_prereleases` the newest of the recent releases list.
    /// `None` when there is none yet.
    pub async fn newest(&self, include_prereleases: bool) -> Result<Option<Release>> {
        if include_prereleases {
            let url = format!("{}/repos/{REPO}/releases?per_page=30", self.base);
            let list: Vec<Release> = self.get_json(&url).await?.unwrap_or_default();
            Ok(newest(&list, true).cloned())
        } else {
            let url = format!("{}/repos/{REPO}/releases/latest", self.base);
            Ok(self.get_json::<Release>(&url).await?.filter(|r| !r.draft && !r.prerelease))
        }
    }

    /// The newest release when it is newer than `current`.
    pub async fn update_for(&self, current: &str, include_prereleases: bool) -> Result<Option<Release>> {
        Ok(self.newest(include_prereleases).await?.filter(|r| is_newer(&r.tag_name, current)))
    }

    /// A small text asset (`SHA256SUMS`).
    pub async fn text(&self, url: &str) -> Result<String> {
        Ok(self.client.get(url).send().await?.error_for_status()?.text().await?)
    }
}

/// When the CLI last looked for a new version (`<app dir>/update_check.json`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckStamp {
    /// Unix time, seconds.
    pub checked_at: u64,
    /// The newest version seen then.
    #[serde(default)]
    pub latest: Option<String>,
}

/// File name of [`CheckStamp`] in the app directory.
pub const CHECK_STAMP_FILE: &str = "update_check.json";

impl CheckStamp {
    pub fn load(path: &Path) -> Self {
        std::fs::read(path).ok().and_then(|raw| serde_json::from_slice(&raw).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        crate::paths::write_atomic(path, &serde_json::to_vec(self)?)?;
        Ok(())
    }

    /// `true` when there was no check yet, or the last one is at least `interval`
    /// old (or in the future: a clock set back doesn't silence the check forever).
    pub fn due(&self, now_secs: u64, interval: Duration) -> bool {
        self.checked_at == 0 || now_secs < self.checked_at || now_secs - self.checked_at >= interval.as_secs()
    }
}

/// `HOMEFI_NO_UPDATE_CHECK` is set to something other than empty / `0` / `false`.
pub fn disabled_by_env() -> bool {
    std::env::var(NO_UPDATE_CHECK_ENV)
        .map(|v| {
            let v = v.trim();
            !(v.is_empty() || v == "0" || v.eq_ignore_ascii_case("false"))
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        s.parse().unwrap()
    }

    #[test]
    fn versions_parse_and_print() {
        assert_eq!(v("1.2.3"), Version { major: 1, minor: 2, patch: 3, pre: vec![] });
        assert_eq!(v("v1.2.3").to_string(), "1.2.3");
        assert_eq!(v("1.5.0-beta.1").to_string(), "1.5.0-beta.1");
        assert_eq!(v("1.5.0-beta.1+abc").to_string(), "1.5.0-beta.1");
        assert_eq!(v(" 2.0.0 "), v("2.0.0"));
        for bad in ["", "1", "1.2", "1.2.3.4", "1.x.3", "1.2.3-", "1.2.3-a..b", "latest"] {
            assert!(bad.parse::<Version>().is_err(), "{bad}");
        }
    }

    #[test]
    fn versions_follow_semver_precedence() {
        let order = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
            "1.0.1",
            "1.1.0",
            "1.10.0",
            "2.0.0",
        ];
        for pair in order.windows(2) {
            assert!(v(pair[0]) < v(pair[1]), "{} < {}", pair[0], pair[1]);
        }
        assert!(is_newer("v1.2.0", "1.1.0"));
        assert!(!is_newer("1.1.0", "1.1.0"));
        assert!(!is_newer("1.2.0-beta.1", "1.2.0"));
        assert!(is_newer("1.2.0", "1.2.0-beta.3"));
        assert!(!is_newer("garbage", "1.0.0"));
        assert!(!is_newer("9.9.9", "not a version"));
    }

    fn release(tag: &str, pre: bool) -> Release {
        Release {
            tag_name: tag.into(),
            name: None,
            body: None,
            prerelease: pre,
            draft: false,
            html_url: format!("{RELEASES_PAGE}/tag/{tag}"),
            published_at: None,
            assets: vec![],
        }
    }

    #[test]
    fn newest_skips_drafts_and_prereleases_unless_asked() {
        let mut draft = release("v3.0.0", false);
        draft.draft = true;
        let list = vec![
            release("v1.1.0", false),
            release("v1.3.0-beta.1", true),
            release("v1.2.0", false),
            draft,
            release("nightly", false),
        ];
        assert_eq!(newest(&list, false).unwrap().tag_name, "v1.2.0");
        assert_eq!(newest(&list, true).unwrap().tag_name, "v1.3.0-beta.1");
        // a tag with a pre-release suffix that wasn't flagged as one is still a pre-release
        let sneaky = vec![release("v1.2.0", false), release("v1.3.0-rc.1", false)];
        assert_eq!(newest(&sneaky, false).unwrap().tag_name, "v1.2.0");
        assert!(newest(&[], true).is_none());
    }

    #[test]
    fn notes_drop_the_legal_notice() {
        let body = "### Added\n\n- Updates\n\n---\n**Personal use only.**\n";
        assert_eq!(release_notes(body), "### Added\n\n- Updates");
        assert_eq!(release_notes("  just text \n"), "just text");
        assert_eq!(release_notes("---\nnotice only"), "");
        assert_eq!(release_notes("a\r\n---\r\nb"), "a");
    }

    #[test]
    fn sha256sums_are_read_like_sha256sum_writes_them() {
        let h1 = "a".repeat(64);
        let h2 = "B".repeat(64);
        let sums = format!("{h1}  HomeFi-android.apk\n{h2} *HomeFi-macos-arm64.dmg\nshort  x\n");
        assert_eq!(sha256_of(&sums, "HomeFi-android.apk"), Some(h1));
        assert_eq!(sha256_of(&sums, "HomeFi-macos-arm64.dmg"), Some("b".repeat(64)));
        assert_eq!(sha256_of(&sums, "x"), None);
        assert_eq!(sha256_of(&sums, "HomeFi-android"), None);
    }

    #[test]
    fn the_cli_checks_once_a_day() {
        let stamp = CheckStamp { checked_at: 1_000_000, latest: None };
        assert!(!stamp.due(1_000_000 + 3600, CLI_CHECK_INTERVAL));
        assert!(stamp.due(1_000_000 + 86_400, CLI_CHECK_INTERVAL));
        assert!(stamp.due(10, CLI_CHECK_INTERVAL), "a clock set back checks again");
        assert!(CheckStamp::default().due(5, CLI_CHECK_INTERVAL));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CHECK_STAMP_FILE);
        assert_eq!(CheckStamp::load(&path), CheckStamp::default());
        let saved = CheckStamp { checked_at: 42, latest: Some("1.2.0".into()) };
        saved.save(&path).unwrap();
        assert_eq!(CheckStamp::load(&path), saved);
    }
}
