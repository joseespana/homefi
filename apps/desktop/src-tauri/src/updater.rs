//! "HomeFi 1.2.0 is available": in-app updates from the GitHub release.
//!
//! Installs that can update themselves (the macOS app, the Windows setup / MSI,
//! the Linux AppImage, `.deb` and `.rpm`) use tauri-plugin-updater with the
//! release's `latest.json` and minisign signatures (public key in
//! `tauri.conf.json`). Anything else (a dev build, the app run from the disk
//! image or a translocated copy, an AppImage without `$APPIMAGE`, a package
//! without `pkexec`) only says a new version is out, with a link to the release
//! page. The only calls are to GitHub (the API and the release assets).
//!
//! Nothing interrupts the music without asking: the update downloads in the
//! background, and the restart waits for the user ("Restart now" / "When the
//! song ends"), or for HomeFi to be idle when the settings say so.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use homefi_core::update::{self, ReleaseFeed, RELEASES_PAGE};
use homefi_player::{Snapshot, Status};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::commands::AppState;
use crate::settings::UpdateInstall;

/// Manifest of the newest stable release (`/latest` skips pre-releases).
pub const STABLE_ENDPOINT: &str = "https://github.com/joseespana/homefi/releases/latest/download/latest.json";
/// Points the updater at another `latest.json` (a local test server, see docs/releasing.md).
pub const ENDPOINT_ENV: &str = "HOMEFI_UPDATE_ENDPOINT";
/// When the last check ran (`<app dir>/desktop_update.json`).
const STATE_FILE: &str = "desktop_update.json";
/// The first check waits a moment after launch, so it doesn't compete with the library scan.
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(20);
const TICK: Duration = Duration::from_secs(30);
/// "Automatically when HomeFi is idle": nothing playing, downloading or shared for this long.
const IDLE_FOR: Duration = Duration::from_secs(5 * 60);

/// How this copy of HomeFi can be updated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Install {
    /// In place, by the updater: `app`, `nsis`, `msi`, `appimage`, `deb` or `rpm`.
    InPlace(&'static str),
    /// Only "Download" (the release page), and why.
    Manual(&'static str),
}

/// What the UI shows (event `update`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateView {
    pub current: String,
    /// `idle`, `checking`, `upToDate`, `available`, `downloading`, `ready`, `installing` or `error`.
    pub status: &'static str,
    pub version: Option<String>,
    /// Release notes (Markdown) from `latest.json` / the release.
    pub notes: Option<String>,
    /// Publication date, epoch ms.
    pub date: Option<i64>,
    pub prerelease: bool,
    /// Release page ("What's new" in the browser, "Download" when it can't update itself).
    pub page_url: String,
    /// The update can be installed from the app.
    pub in_place: bool,
    /// `app`, `nsis`, `msi`, `appimage`, `deb`, `rpm` or `manual`.
    pub install_kind: &'static str,
    /// Why it can't update itself: `dev`, `dmg`, `translocated`, `not-app`, `no-appimage`,
    /// `no-pkexec`, `unknown` or `no-package` (the release has none for this system).
    pub manual_reason: Option<&'static str>,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub error: Option<String>,
    /// Epoch ms of the last check.
    pub last_checked: Option<u64>,
    /// "When the song ends" was picked: the restart waits for this song.
    pub after_song: bool,
    /// "Later": the banner stays hidden for this version until the app restarts.
    pub dismissed: bool,
}

#[derive(Default, Serialize, Deserialize)]
struct Persisted {
    last_checked: Option<u64>,
}

struct Inner {
    view: UpdateView,
    update: Option<Update>,
    bytes: Option<Vec<u8>>,
    /// The song playing when "When the song ends" was picked.
    song: Option<Option<PathBuf>>,
    idle_since: Option<Instant>,
    dismissed_version: Option<String>,
}

pub struct Updates {
    inner: Mutex<Inner>,
    state_path: PathBuf,
    install: Install,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

fn release_page(version: &str) -> String {
    format!("{RELEASES_PAGE}/tag/v{version}")
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
}

/// How this copy was installed, from the bundle type the bundler wrote into the
/// binary and where it runs from.
pub fn detect_install() -> Install {
    use tauri::utils::config::BundleType;
    if cfg!(debug_assertions) {
        return Install::Manual("dev");
    }
    let exe = std::env::current_exe().unwrap_or_default();
    let exe = exe.to_string_lossy();
    match tauri::utils::platform::bundle_type() {
        Some(BundleType::Nsis) => Install::InPlace("nsis"),
        Some(BundleType::Msi) => Install::InPlace("msi"),
        Some(BundleType::App | BundleType::Dmg) if cfg!(target_os = "macos") => {
            if !exe.contains(".app/Contents/MacOS/") {
                Install::Manual("not-app")
            } else if exe.contains("/AppTranslocation/") {
                // opened straight from Downloads: macOS runs a read-only copy
                Install::Manual("translocated")
            } else if exe.starts_with("/Volumes/") && !exe.starts_with("/Volumes/Macintosh HD/") {
                Install::Manual("dmg")
            } else {
                Install::InPlace("app")
            }
        }
        Some(BundleType::AppImage) => {
            if std::env::var_os("APPIMAGE").is_some() {
                Install::InPlace("appimage")
            } else {
                Install::Manual("no-appimage")
            }
        }
        // the plugin installs packages with pkexec (a password prompt); without it, no prompt
        Some(BundleType::Deb) if on_path("pkexec") => Install::InPlace("deb"),
        Some(BundleType::Rpm) if on_path("pkexec") => Install::InPlace("rpm"),
        Some(BundleType::Deb | BundleType::Rpm) => Install::Manual("no-pkexec"),
        _ => Install::Manual("unknown"),
    }
}

impl Updates {
    /// `current`: the app's version, as the updater compares it.
    pub fn new(app_dir: &std::path::Path, current: String) -> Self {
        let state_path = app_dir.join(STATE_FILE);
        let persisted: Persisted =
            std::fs::read(&state_path).ok().and_then(|raw| serde_json::from_slice(&raw).ok()).unwrap_or_default();
        let install = detect_install();
        let (in_place, install_kind, manual_reason) = match install {
            Install::InPlace(kind) => (true, kind, None),
            Install::Manual(why) => (false, "manual", Some(why)),
        };
        let view = UpdateView {
            current,
            status: "idle",
            version: None,
            notes: None,
            date: None,
            prerelease: false,
            page_url: RELEASES_PAGE.to_owned(),
            in_place,
            install_kind,
            manual_reason,
            downloaded: 0,
            total: None,
            error: None,
            last_checked: persisted.last_checked,
            after_song: false,
            dismissed: false,
        };
        Self {
            inner: Mutex::new(Inner {
                view,
                update: None,
                bytes: None,
                song: None,
                idle_since: None,
                dismissed_version: None,
            }),
            state_path,
            install,
        }
    }

    pub fn view(&self) -> UpdateView {
        self.inner.lock().unwrap().view.clone()
    }

    fn change(&self, app: &AppHandle, f: impl FnOnce(&mut Inner)) -> UpdateView {
        let view = {
            let mut inner = self.inner.lock().unwrap();
            f(&mut inner);
            inner.view.dismissed = inner.view.version.is_some() && inner.view.version == inner.dismissed_version;
            inner.view.clone()
        };
        let _ = app.emit("update", &view);
        view
    }

    fn save_checked(&self, at: u64) {
        let raw = serde_json::to_vec(&Persisted { last_checked: Some(at) }).unwrap_or_default();
        if let Err(e) = homefi_core::paths::write_atomic(&self.state_path, &raw) {
            tracing::warn!("could not save the update check time: {e}");
        }
    }
}

/// Plain words for the screen; the details go to the log.
fn humane(e: &tauri_plugin_updater::Error) -> String {
    use tauri_plugin_updater::Error as E;
    tracing::warn!("update: {e}");
    match e {
        E::Minisign(_)
        | E::SignatureUtf8(_)
        | E::Base64(_)
        | E::SignedVersionMismatch { .. }
        | E::MissingSignedVersion => {
            "The update's signature didn't match, so it wasn't installed. Download it from the release page instead."
                .into()
        }
        E::Reqwest(_) | E::Network(_) => "Couldn't reach GitHub. Check your internet connection and try again.".into(),
        E::Io(_) | E::TempDirNotFound | E::TempDirNotOnSameMountPoint | E::PackageInstallFailed => {
            "The update couldn't be installed here. Download it from the release page instead.".into()
        }
        _ => "Couldn't check for updates right now. Try again later.".into(),
    }
}

/// The `latest.json` to ask: the newest pre-release's own, when the settings
/// include them and it is newer than the newest stable, then the stable one.
async fn endpoints(prereleases: bool) -> Vec<url::Url> {
    if let Some(url) = std::env::var(ENDPOINT_ENV).ok().and_then(|u| u.parse().ok()) {
        return vec![url];
    }
    let mut urls = Vec::new();
    if prereleases {
        match ReleaseFeed::new("HomeFi-desktop").newest(true).await {
            Ok(Some(r)) if r.prerelease => {
                if let Some(a) = r.asset(update::DESKTOP_MANIFEST) {
                    urls.extend(a.browser_download_url.parse().ok());
                }
            }
            Ok(_) => {}
            Err(e) => tracing::warn!("could not list the pre-releases: {e}"),
        }
    }
    urls.extend(STABLE_ENDPOINT.parse().ok());
    urls
}

/// Looks for a new version (the "Check now" button, the launch / 6-hourly check).
pub async fn check(app: &AppHandle) -> UpdateView {
    let updates = app.state::<Updates>();
    {
        let inner = updates.inner.lock().unwrap();
        // already found and downloading / downloaded: nothing new to ask
        if matches!(inner.view.status, "checking" | "downloading" | "ready" | "installing") {
            return inner.view.clone();
        }
    }
    updates.change(app, |i| {
        i.view.status = "checking";
        i.view.error = None;
    });
    let prereleases = app.state::<AppState>().settings().updates.prereleases;

    let mut found: Option<Update> = None;
    let mut failure: Option<String> = None;
    if let Install::InPlace(_) = updates.install {
        let urls = endpoints(prereleases).await;
        let handle = app.clone();
        let built = app.updater_builder().endpoints(urls).and_then(|b| {
            b.on_before_exit(move || {
                // Windows: the installer takes over, the app quits right after this
                crate::share::shutdown(&handle);
                handle.state::<AppState>().flush_settings();
            })
            .build()
        });
        match built {
            Ok(updater) => match updater.check().await {
                Ok(update) => found = update,
                Err(e) => failure = Some(humane(&e)),
            },
            Err(e) => failure = Some(humane(&e)),
        }
    }

    let checked = now_ms();
    updates.save_checked(checked);
    if let Some(update) = found {
        let version = update.version.clone();
        return updates.change(app, |i| {
            i.view.status = "available";
            i.view.prerelease = version.contains('-');
            i.view.page_url = release_page(&version);
            i.view.notes = update.body.clone().map(|n| update::release_notes(&n));
            i.view.date = update.date.map(|d| d.unix_timestamp() * 1000);
            i.view.version = Some(version);
            i.view.last_checked = Some(checked);
            i.view.downloaded = 0;
            i.view.total = None;
            i.update = Some(update);
            i.bytes = None;
        });
    }

    // can't update itself (or its manifest failed): the GitHub release says what is out
    let release = match (updates.install, &failure) {
        (Install::InPlace(_), None) => Ok(None),
        _ => ReleaseFeed::new("HomeFi-desktop").update_for(&updates.view().current, prereleases).await,
    };
    match release {
        Ok(Some(r)) => updates.change(app, |i| {
            i.view.status = "available";
            i.view.version = Some(r.version_string());
            i.view.prerelease = r.prerelease;
            i.view.notes = Some(r.notes());
            i.view.date = None;
            i.view.page_url = r.html_url.clone();
            i.view.last_checked = Some(checked);
            // the installer's manifest failed but a release is out: "Download" it
            if i.view.in_place {
                i.view.in_place = false;
                i.view.install_kind = "manual";
                i.view.manual_reason = Some("no-package");
            }
            i.update = None;
        }),
        Ok(None) => updates.change(app, |i| {
            i.view.status = if failure.is_some() { "error" } else { "upToDate" };
            i.view.error = failure.clone();
            i.view.version = None;
            i.view.notes = None;
            i.view.last_checked = Some(checked);
        }),
        Err(e) => {
            tracing::warn!("update check: {e}");
            updates.change(app, |i| {
                i.view.status = "error";
                i.view.error =
                    Some(failure.unwrap_or_else(|| {
                        "Couldn't reach GitHub. Check your internet connection and try again.".into()
                    }));
                i.view.last_checked = Some(checked);
            })
        }
    }
}

/// Downloads the update found by [`check`] and verifies its signature ("Update now").
pub async fn download(app: &AppHandle) -> Result<(), String> {
    let updates = app.state::<Updates>();
    let update = {
        let inner = updates.inner.lock().unwrap();
        if inner.view.status != "available" && inner.view.status != "error" {
            return Ok(());
        }
        inner.update.clone().ok_or("This update can only be downloaded from the release page.")?
    };
    updates.change(app, |i| {
        i.view.status = "downloading";
        i.view.downloaded = 0;
        i.view.total = None;
        i.view.error = None;
    });
    let mut got = 0u64;
    let mut last = Instant::now();
    let progress_app = app.clone();
    let result = update
        .download(
            |chunk, total| {
                got += chunk as u64;
                // a few updates a second are plenty for a progress bar
                if last.elapsed() >= Duration::from_millis(150) {
                    last = Instant::now();
                    progress_app.state::<Updates>().change(&progress_app, |i| {
                        i.view.downloaded = got;
                        i.view.total = total;
                    });
                }
            },
            || {},
        )
        .await;
    match result {
        Ok(bytes) => {
            let size = bytes.len() as u64;
            updates.change(app, |i| {
                i.view.status = "ready";
                i.view.downloaded = size;
                i.view.total = Some(size);
                i.bytes = Some(bytes);
            });
            Ok(())
        }
        Err(e) => {
            let text = humane(&e);
            updates.change(app, |i| {
                i.view.status = "error";
                i.view.error = Some(text.clone());
            });
            Err(text)
        }
    }
}

/// Installs the downloaded update and starts the new version. Windows: the
/// installer closes HomeFi and opens it again by itself.
fn install_and_restart(app: &AppHandle) {
    let updates = app.state::<Updates>();
    let (update, bytes) = {
        let mut inner = updates.inner.lock().unwrap();
        match (inner.update.clone(), inner.bytes.take()) {
            (Some(u), Some(b)) => (u, b),
            (_, bytes) => {
                inner.bytes = bytes;
                return;
            }
        }
    };
    updates.change(app, |i| {
        i.view.status = "installing";
        i.song = None;
        i.view.after_song = false;
    });
    let app = app.clone();
    // blocking: macOS may ask for an administrator password on the main thread
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<AppState>().player.pause();
        match update.install(&bytes) {
            // macOS / Linux: the new version is in place; the Exit event says goodbye to guests
            Ok(()) => app.restart(),
            Err(e) => {
                let text = humane(&e);
                app.state::<Updates>().change(&app, |i| {
                    i.view.status = "error";
                    i.view.error = Some(text);
                    // the bytes are still good: "Restart" can be tried again
                    i.bytes = Some(bytes);
                });
            }
        }
    });
}

/// "Restart now" (`now`) or "When the song ends" (`song_end`).
pub fn restart(app: &AppHandle, when: &str) -> Result<(), String> {
    let updates = app.state::<Updates>();
    if updates.view().status != "ready" {
        return Err("The update isn't downloaded yet.".into());
    }
    let state = app.state::<AppState>();
    let snapshot = state.player.snapshot();
    if when == "song_end" && snapshot.status == Status::Playing {
        let song = snapshot.current.and_then(|i| snapshot.queue.get(i).cloned());
        updates.change(app, |i| {
            i.song = Some(song);
            i.view.after_song = true;
        });
        return Ok(());
    }
    install_and_restart(app);
    Ok(())
}

/// "Later": hides the banner for this version until HomeFi starts again.
pub fn dismiss(app: &AppHandle) {
    let updates = app.state::<Updates>();
    updates.change(app, |i| {
        i.dismissed_version = i.view.version.clone();
        i.song = None;
        i.view.after_song = false;
    });
}

/// Every player change: the restart waiting for "the song ends" goes once the
/// song changed or the music stopped.
pub fn on_player(app: &AppHandle, snapshot: &Snapshot) {
    let Some(updates) = app.try_state::<Updates>() else { return };
    let waiting = {
        let inner = updates.inner.lock().unwrap();
        match &inner.song {
            Some(song) => {
                let now = snapshot.current.and_then(|i| snapshot.queue.get(i));
                snapshot.status != Status::Playing || now != song.as_ref()
            }
            None => false,
        }
    };
    if waiting {
        install_and_restart(app);
    }
}

/// HomeFi has nothing going on: no music, no downloads, no shared session.
fn is_busy(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    state.player.snapshot().status == Status::Playing
        || state.cancel.lock().unwrap().is_some()
        || app.state::<crate::share::Share>().in_session()
}

/// Checks on launch and every 6 hours (when on in the settings); with "When
/// HomeFi is idle", downloads by itself and restarts after 5 idle minutes.
pub async fn run(app: AppHandle) {
    tokio::time::sleep(FIRST_CHECK_AFTER).await;
    let mut launched = true;
    loop {
        let settings = app.state::<AppState>().settings().updates;
        let updates = app.state::<Updates>();
        let last = updates.view().last_checked.unwrap_or(0);
        let due = now_ms().saturating_sub(last) >= update::CHECK_INTERVAL.as_millis() as u64;
        if settings.auto_check && (launched || due) {
            check(&app).await;
        }
        launched = false;

        if settings.install == UpdateInstall::Idle {
            let view = updates.view();
            if view.status == "available" && view.in_place && !view.dismissed {
                let _ = download(&app).await;
            }
            let busy = is_busy(&app);
            let idle_long = {
                let mut inner = updates.inner.lock().unwrap();
                if busy || inner.view.status != "ready" {
                    inner.idle_since = None;
                    false
                } else {
                    inner.idle_since.get_or_insert_with(Instant::now).elapsed() >= IDLE_FOR
                }
            };
            if idle_long {
                install_and_restart(&app);
            }
        }
        tokio::time::sleep(TICK).await;
    }
}

// ----------------------------------------------------------------- commands

#[tauri::command(async)]
pub fn update_status(app: AppHandle) -> UpdateView {
    app.state::<Updates>().view()
}

#[tauri::command]
pub async fn update_check(app: AppHandle) -> UpdateView {
    // asked by hand: the banner shows again even after "Later"
    app.state::<Updates>().change(&app, |i| i.dismissed_version = None);
    check(&app).await
}

#[tauri::command]
pub async fn update_download(app: AppHandle) -> Result<(), String> {
    download(&app).await
}

#[tauri::command(async)]
pub fn update_restart(app: AppHandle, when: String) -> Result<(), String> {
    restart(&app, &when)
}

#[tauri::command(async)]
pub fn update_dismiss(app: AppHandle) {
    dismiss(&app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_builds_never_update_themselves() {
        // tests are debug builds: only "Download" is offered
        assert_eq!(detect_install(), Install::Manual("dev"));
    }

    #[test]
    fn the_release_page_is_the_tag() {
        assert_eq!(release_page("1.2.0"), "https://github.com/joseespana/homefi/releases/tag/v1.2.0");
    }
}
