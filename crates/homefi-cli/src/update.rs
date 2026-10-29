//! "A new version is available": a quiet one-line notice on stderr, at most once
//! a day. It never installs anything. Skipped when stderr is not a terminal,
//! with `HOMEFI_NO_UPDATE_CHECK=1` or `update_check = false` in the config. The
//! only call is to GitHub's public releases API.

use std::io::IsTerminal;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use console::style;
use homefi_core::session::Session;
use homefi_core::update::{self, CheckStamp, ReleaseFeed};
use tokio::task::JoinHandle;

/// How long a finished command waits for the check before giving up on it.
const WAIT: Duration = Duration::from_secs(2);

/// A check running next to the command (it never slows the command down).
pub struct Notice(Option<JoinHandle<Option<String>>>);

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Starts the check when it is due.
pub fn start(session: &Session) -> Notice {
    if !session.config().update_check || update::disabled_by_env() || !std::io::stderr().is_terminal() {
        return Notice(None);
    }
    let path = session.app_dir().join(update::CHECK_STAMP_FILE);
    let now = now_secs();
    let stamp = CheckStamp::load(&path);
    if !stamp.due(now, update::CLI_CHECK_INTERVAL) {
        return Notice(None);
    }
    // stamped before asking: offline, the next commands don't try again all day
    let _ = CheckStamp { checked_at: now, latest: stamp.latest.clone() }.save(&path);
    Notice(Some(tokio::spawn(async move {
        let release = match ReleaseFeed::new("HomeFi-cli").newest(false).await {
            Ok(release) => release?,
            Err(e) => {
                tracing::debug!("update check failed: {e}");
                return None;
            }
        };
        let version = release.version_string();
        let _ = CheckStamp { checked_at: now, latest: Some(version.clone()) }.save(&path);
        update::is_newer(&version, homefi_core::VERSION)
            .then(|| format!("A new version {version} is available: {}", release.html_url))
    })))
}

impl Notice {
    /// Prints the notice when the check found a newer version in time.
    pub async fn finish(self) {
        let Some(task) = self.0 else { return };
        if let Ok(Ok(Some(text))) = tokio::time::timeout(WAIT, task).await {
            eprintln!("{}", style(text).dim());
        }
    }
}
