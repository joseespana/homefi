//! Discs (`src/lib/api.ts`, Settings → Discs): thin commands over `homefi-disc`. Every rule
//! (reading, offsets, encoders, names, tags) lives in that crate; this file only moves the
//! drive's work off the UI thread, sends progress through a channel and tells the screen when
//! a disc goes in or out. Reading the drive blocks, so it never runs on the UI thread or a
//! tokio worker: `spawn_blocking` for the short jobs, a thread of its own for a rip.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use homefi_core::humane::{self, Lang};
use homefi_disc::{
    DiscError, DiscFacts, DiscMatch, DiscSettings, DriveId, DriveInfo, Media, ProtectionKind, RipFormat, RipOptions,
    RipOutput, RipProgress, RipRequest, RipSummary, SectorSource, Toc,
};
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::{err, AppState};

type CmdResult<T> = Result<T, String>;

/// A cover picture chosen by the user is a few MB at most.
const MAX_COVER_FILE_BYTES: u64 = 20 * 1024 * 1024;
const JPEG_MAGIC: [u8; 2] = [0xFF, 0xD8];
const PNG_MAGIC: [u8; 4] = [0x89, b'P', b'N', b'G'];
/// How long the watcher waits before the first look, so startup is not slowed by the drive.
const WATCH_START_DELAY: Duration = Duration::from_secs(2);

// ----------------------------------------------------------------- what the screen sees

/// A drive and what is in it (`DriveView` in `api.ts`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveView {
    id: String,
    name: String,
    media: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    tracks: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    length_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    protection: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mount: Option<String>,
}

impl DriveView {
    fn of(info: &DriveInfo) -> Self {
        let mut view = DriveView {
            id: info.id.0.clone(),
            name: info.name.clone(),
            media: "none",
            tracks: None,
            length_ms: None,
            protection: None,
            mount: None,
        };
        match &info.media {
            Media::None => {}
            Media::AudioCd { tracks, length_ms } => {
                view.media = "audio";
                view.tracks = Some(*tracks);
                view.length_ms = Some(*length_ms);
            }
            Media::DataDisc { mount } => {
                view.media = "data";
                view.mount = mount.as_ref().map(|m| m.display().to_string());
            }
            Media::Protected(kind) => {
                view.media = "protected";
                view.protection = Some(match kind {
                    ProtectionKind::DvdCss => "css",
                    ProtectionKind::DvdCppm => "cppm",
                    ProtectionKind::BluRayAacs => "aacs",
                });
            }
            Media::Unknown => view.media = "unknown",
        }
        view
    }

    /// There is a disc the user should hear about.
    fn has_disc(&self) -> bool {
        matches!(self.media, "audio" | "data" | "protected")
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TocTrackView {
    number: u8,
    start_lba: u32,
    end_lba: u32,
    pre_emphasis: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TocView {
    tracks: Vec<TocTrackView>,
    leadout_lba: u32,
}

#[derive(Debug, Serialize)]
struct IdsView {
    musicbrainz: String,
    freedb: String,
    ctdb: String,
}

/// An opened audio CD (`DiscView` in `api.ts`).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscView {
    drive: DriveView,
    toc: TocView,
    ids: IdsView,
    mcn: Option<String>,
    cd_text: Option<DiscMatch>,
}

/// What the `disc` event carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum DiscEvent {
    Inserted { drive: DriveView },
    Removed { drive: DriveView },
    Isrcs { drive: DriveView, isrcs: Vec<Option<String>> },
}

/// What the screen asks for when it rips (`RipRequest` in `api.ts`).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RipArgs {
    drive: String,
    tracks: Vec<u8>,
    meta: DiscMatch,
    format: RipFormat,
    /// An image file chosen for the cover.
    cover_file: Option<PathBuf>,
}

fn view_of_toc(toc: &Toc) -> TocView {
    TocView {
        tracks: toc
            .audio_tracks()
            .map(|t| TocTrackView {
                number: t.number,
                start_lba: t.start_lba,
                end_lba: t.end_lba,
                pre_emphasis: t.pre_emphasis,
            })
            .collect(),
        leadout_lba: toc.audio_end_lba(),
    }
}

// ----------------------------------------------------------------- state

/// Disc state of the app: the lookup client, who is using the drive and the cancel switches.
pub struct Disc {
    http: reqwest::Client,
    musicbrainz: homefi_disc::MusicBrainz,
    /// Cancel switch of the running rip (`Some` while one runs).
    rip: Mutex<Option<Arc<AtomicBool>>>,
    /// Stops the ISRC reading of the disc that is open on screen.
    isrcs: Mutex<Option<Arc<AtomicBool>>>,
    /// One user of the drive at a time (opening, ISRCs, ripping).
    drive: Arc<Mutex<()>>,
    /// What the watcher saw last, by drive id.
    seen: Mutex<Option<HashMap<String, DriveView>>>,
    /// Drive names by id, for the rip log.
    names: Mutex<HashMap<String, String>>,
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Disc {
    pub fn new() -> Self {
        let http = reqwest::Client::new();
        Disc {
            musicbrainz: homefi_disc::MusicBrainz::new(http.clone()),
            http,
            rip: Mutex::new(None),
            isrcs: Mutex::new(None),
            drive: Arc::new(Mutex::new(())),
            seen: Mutex::new(None),
            names: Mutex::new(HashMap::new()),
        }
    }

    fn remember_names<'a>(&self, drives: impl IntoIterator<Item = &'a DriveView>) {
        let mut names = locked(&self.names);
        for drive in drives {
            names.insert(drive.id.clone(), drive.name.clone());
        }
    }

    /// Registers the running rip; only one at a time.
    fn begin(&self) -> CmdResult<Arc<AtomicBool>> {
        let mut slot = locked(&self.rip);
        if slot.is_some() {
            return Err(text(Words::AlreadyRipping));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        *slot = Some(cancel.clone());
        Ok(cancel)
    }

    /// Clears the switch of a finished rip.
    fn finish(&self, cancel: &Arc<AtomicBool>) {
        let mut slot = locked(&self.rip);
        if slot.as_ref().is_some_and(|current| Arc::ptr_eq(current, cancel)) {
            *slot = None;
        }
    }

    fn cancel(&self) {
        if let Some(cancel) = locked(&self.rip).as_ref() {
            cancel.store(true, Ordering::Relaxed);
        }
    }

    fn ripping(&self) -> bool {
        locked(&self.rip).is_some()
    }

    /// A new job replaces the ISRC reading of the previous disc.
    fn restart_isrcs(&self) -> Arc<AtomicBool> {
        let fresh = Arc::new(AtomicBool::new(false));
        if let Some(previous) = locked(&self.isrcs).replace(fresh.clone()) {
            previous.store(true, Ordering::Relaxed);
        }
        fresh
    }

    fn stop_isrcs(&self) {
        if let Some(current) = locked(&self.isrcs).as_ref() {
            current.store(true, Ordering::Relaxed);
        }
    }
}

impl Default for Disc {
    fn default() -> Self {
        Self::new()
    }
}

// ----------------------------------------------------------------- words

enum Words {
    AlreadyRipping,
    BadCover,
    CoverUnreadable,
}

fn text(words: Words) -> String {
    let es = humane::language() == Lang::Es;
    match (words, es) {
        (Words::AlreadyRipping, false) => "A disc is already being ripped.".into(),
        (Words::AlreadyRipping, true) => "Ya se está copiando un disco.".into(),
        (Words::BadCover, false) => "The cover must be a JPEG or PNG picture.".into(),
        (Words::BadCover, true) => "La carátula debe ser una imagen JPEG o PNG.".into(),
        (Words::CoverUnreadable, false) => "The cover picture couldn't be read.".into(),
        (Words::CoverUnreadable, true) => "No se pudo leer la imagen de la carátula.".into(),
    }
}

fn problem(error: DiscError) -> String {
    tracing::debug!(%error, "disc problem");
    error.humane(humane::language())
}

/// Runs a drive job on the blocking pool.
async fn blocking<T, F>(job: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, DiscError> + Send + 'static,
{
    tokio::task::spawn_blocking(job).await.map_err(err)?.map_err(problem)
}

// ----------------------------------------------------------------- commands

#[tauri::command]
pub async fn disc_drives(disc: State<'_, Disc>) -> CmdResult<Vec<DriveView>> {
    let drives = blocking(homefi_disc::list_drives).await?;
    let views: Vec<DriveView> = drives.iter().map(DriveView::of).collect();
    disc.remember_names(&views);
    Ok(views)
}

/// The drive asked for, or the first one with an audio CD in it.
fn pick_drive(drives: Vec<DriveInfo>, wanted: Option<&str>) -> Result<DriveInfo, DiscError> {
    let mut candidates = drives.into_iter();
    match wanted {
        Some(id) => candidates.find(|d| d.id.0 == id).ok_or(DiscError::NoDrive),
        None => {
            let all: Vec<DriveInfo> = candidates.collect();
            let first_audio = all.iter().position(|d| matches!(d.media, Media::AudioCd { .. }));
            match (first_audio, all.is_empty()) {
                (Some(i), _) => Ok(all.into_iter().nth(i).expect("index found")),
                (None, true) => Err(DiscError::NoDrive),
                (None, false) => Ok(all.into_iter().next().expect("not empty")),
            }
        }
    }
}

fn open_view(info: DriveInfo, cd_text: bool) -> Result<DiscView, DiscError> {
    match &info.media {
        Media::AudioCd { .. } => {}
        Media::None => return Err(DiscError::NoDisc),
        other => return Err(DiscError::NotAudioCd(other.clone())),
    }
    let mut drive = homefi_disc::open(&info.id)?;
    let toc = drive.toc()?;
    let text = if cd_text { drive.cd_text().ok().flatten() } else { None };
    let mcn = drive.mcn().ok().flatten();
    Ok(DiscView {
        drive: DriveView::of(&info),
        toc: view_of_toc(&toc),
        ids: IdsView { musicbrainz: toc.musicbrainz_id(), freedb: toc.freedb_id(), ctdb: toc.ctdb_toc() },
        mcn,
        cd_text: text.and_then(|t| homefi_disc::identify::from_cd_text(&t, &toc)),
    })
}

/// Opens the audio CD (the drive asked for, or the first one with a CD). ISRCs cost a second a
/// track, so they follow in a `disc` event.
#[tauri::command]
pub async fn disc_open(app: AppHandle, drive: Option<String>) -> CmdResult<DiscView> {
    let state = app.state::<AppState>();
    let disc = app.state::<Disc>();
    if disc.ripping() {
        return Err(text(Words::AlreadyRipping));
    }
    let use_cd_text = state.settings().disc.sources.cd_text;
    let lock = disc.drive.clone();
    let cancel_isrcs = disc.restart_isrcs();
    let view = blocking(move || {
        let _one_user = locked(&lock);
        let info = pick_drive(homefi_disc::list_drives()?, drive.as_deref())?;
        open_view(info, use_cd_text)
    })
    .await?;
    disc.remember_names([&view.drive]);

    let tracks: Vec<u8> = view.toc.tracks.iter().map(|t| t.number).collect();
    let (id, event_drive, lock, handle) =
        (DriveId(view.drive.id.clone()), view.drive.clone(), disc.drive.clone(), app.clone());
    tauri::async_runtime::spawn_blocking(move || {
        let isrcs = read_isrcs(&lock, &cancel_isrcs, &id, &tracks);
        if let Some(isrcs) = isrcs {
            let _ = handle.emit("disc", DiscEvent::Isrcs { drive: event_drive, isrcs });
        }
    });
    Ok(view)
}

/// ISRC of every track; `None` when stopped or the drive can't be used any more.
fn read_isrcs(lock: &Mutex<()>, cancel: &AtomicBool, id: &DriveId, tracks: &[u8]) -> Option<Vec<Option<String>>> {
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    let _one_user = locked(lock);
    let mut drive = homefi_disc::open(id).ok()?;
    let mut isrcs = Vec::with_capacity(tracks.len());
    for track in tracks {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        isrcs.push(drive.isrc(*track).ok().flatten());
    }
    Some(isrcs)
}

/// Names for the disc from the places the user allowed: its own text, and MusicBrainz only
/// when "Look up discs on MusicBrainz" is on.
#[tauri::command]
pub async fn disc_identify(
    app: AppHandle,
    drive: String,
    sources: homefi_disc::settings::Sources,
) -> CmdResult<Vec<DiscMatch>> {
    let state = app.state::<AppState>();
    let disc = app.state::<Disc>();
    let allowed = state.settings().disc.sources;
    // the saved switch is the user's decision; the screen only asks
    let sources = homefi_disc::settings::Sources {
        cd_text: sources.cd_text && allowed.cd_text,
        musicbrainz: sources.musicbrainz && allowed.musicbrainz,
    };
    let lock = disc.drive.clone();
    let (toc, text) = blocking(move || {
        let _one_user = locked(&lock);
        let mut drive = homefi_disc::open(&DriveId(drive))?;
        let toc = drive.toc()?;
        let text = if sources.cd_text { drive.cd_text().ok().flatten() } else { None };
        Ok((toc, text))
    })
    .await?;
    let facts = DiscFacts { toc: &toc, cd_text: text.as_ref() };
    Ok(homefi_disc::identify(&facts, &sources, &disc.musicbrainz).await)
}

/// The cover for the files: the picture the user chose, else the one MusicBrainz found.
async fn cover_for(disc: &Disc, args: &RipArgs, settings: &DiscSettings) -> CmdResult<Option<Vec<u8>>> {
    if let Some(file) = &args.cover_file {
        return read_cover_file(file).await.map(Some);
    }
    use homefi_disc::settings::CoverSource;
    match (&args.meta.cover, settings.cover.source) {
        (Some(homefi_disc::CoverRef::Url { url }), CoverSource::Caa) => {
            Ok(homefi_disc::fetch_cover(&disc.http, url).await)
        }
        _ => Ok(None),
    }
}

async fn read_cover_file(path: &Path) -> CmdResult<Vec<u8>> {
    let path = homefi_core::paths::expand_tilde(path);
    let size = tokio::fs::metadata(&path).await.map_err(|_| text(Words::CoverUnreadable))?.len();
    if size > MAX_COVER_FILE_BYTES {
        return Err(text(Words::BadCover));
    }
    let bytes = tokio::fs::read(&path).await.map_err(|_| text(Words::CoverUnreadable))?;
    if bytes.starts_with(&JPEG_MAGIC) || bytes.starts_with(&PNG_MAGIC) {
        Ok(bytes)
    } else {
        Err(text(Words::BadCover))
    }
}

/// Where the files go and what they are called: the rip folder if set, else the download
/// folder; the rip template if set, else the downloads template.
fn output_for(state: &AppState, settings: &DiscSettings, cover: Option<Vec<u8>>) -> RipOutput {
    let app_settings = state.settings();
    let dir = settings.rip_dir.clone().unwrap_or(app_settings.download_dir);
    let template = settings.template.clone().unwrap_or_else(|| state.session.config().templates.album().to_owned());
    RipOutput { dir: homefi_core::paths::expand_tilde(&dir), template, embed_cover: app_settings.embed_cover, cover }
}

/// Rips the tracks asked for. Progress goes through `on_progress`; the answer comes when the
/// last track is filed. Nothing else uses the drive meanwhile.
#[tauri::command]
pub async fn disc_rip(app: AppHandle, request: RipArgs, on_progress: Channel<RipProgress>) -> CmdResult<RipSummary> {
    let disc = app.state::<Disc>();
    let cancel = disc.begin()?;
    let outcome = rip_job(&app, &disc, request, on_progress, cancel.clone()).await;
    disc.finish(&cancel);
    // what was saved is in the library even when the rip stopped halfway
    if let Err(e) = crate::commands::rescan(&app).await {
        tracing::warn!("library scan after ripping failed: {e}");
    }
    let (summary, eject) = outcome?;
    if let Some(id) = eject {
        // the disc is done: give it back (a failure to eject is not the rip's problem)
        let _ = tokio::task::spawn_blocking(move || homefi_disc::eject(&id)).await;
    }
    Ok(summary)
}

async fn rip_job(
    app: &AppHandle,
    disc: &State<'_, Disc>,
    args: RipArgs,
    on_progress: Channel<RipProgress>,
    cancel: Arc<AtomicBool>,
) -> CmdResult<(RipSummary, Option<DriveId>)> {
    let state = app.state::<AppState>();
    let settings = state.settings().disc;
    let cover = cover_for(disc, &args, &settings).await?;
    let id = DriveId(args.drive.clone());
    let drive_name = locked(&disc.names).get(&id.0).cloned().unwrap_or_else(|| id.0.clone());
    let request = RipRequest {
        drive_name,
        tracks: args.tracks,
        meta: args.meta,
        format: args.format,
        plan: settings.read_plan(&id),
        options: RipOptions::from_settings(&settings),
        out: output_for(&state, &settings, cover),
    };

    // the ISRC reading of the open disc steps aside
    disc.stop_isrcs();
    let lock = disc.drive.clone();
    let runtime = tokio::runtime::Handle::current();
    let (done_tx, done_rx) = tokio::sync::oneshot::channel();
    let worker = {
        let (id, cancel) = (id.clone(), cancel.clone());
        move || {
            let result = run_rip(&lock, &id, &request, &cancel, &runtime, &on_progress);
            let _ = done_tx.send(result);
        }
    };
    std::thread::Builder::new().name("disc-rip".into()).spawn(worker).map_err(err)?;
    let summary = done_rx.await.map_err(|_| text_stopped())?.map_err(problem)?;
    // the disc is given back only when something was saved from it
    let eject = (settings.eject_when_done && summary.ok > 0).then_some(id);
    Ok((summary, eject))
}

fn text_stopped() -> String {
    match humane::language() {
        Lang::Es => "La copia se detuvo de forma inesperada.".into(),
        Lang::En => "The rip stopped unexpectedly.".into(),
    }
}

/// On the `disc-rip` thread: open the drive, check it holds an audio CD, rip.
fn run_rip(
    lock: &Mutex<()>,
    id: &DriveId,
    request: &RipRequest,
    cancel: &AtomicBool,
    runtime: &tokio::runtime::Handle,
    on_progress: &Channel<RipProgress>,
) -> Result<RipSummary, DiscError> {
    let _one_user = locked(lock);
    let mut drive = homefi_disc::open(id)?;
    match drive.media() {
        Media::AudioCd { .. } => {}
        other => return Err(DiscError::NotAudioCd(other)),
    }
    homefi_disc::rip(&mut drive, request, cancel, runtime, &|progress| {
        if let Err(e) = on_progress.send(progress) {
            tracing::debug!("rip progress not delivered: {e}");
        }
    })
}

#[tauri::command]
pub fn disc_cancel(disc: State<'_, Disc>) {
    disc.cancel();
}

#[tauri::command]
pub async fn disc_eject(disc: State<'_, Disc>, drive: String) -> CmdResult<()> {
    if disc.ripping() {
        return Err(text(Words::AlreadyRipping));
    }
    disc.stop_isrcs();
    let lock = disc.drive.clone();
    blocking(move || {
        let _one_user = locked(&lock);
        homefi_disc::eject(&DriveId(drive))
    })
    .await
}

// ----------------------------------------------------------------- the watcher

/// Events for what changed between two looks at the drives. A drive that had a disc and now
/// has none (or another one) sends `Removed`; one that has a disc it didn't have sends `Inserted`.
fn changes(before: &HashMap<String, DriveView>, now: &[DriveView]) -> Vec<DiscEvent> {
    let mut events = Vec::new();
    for old in before.values().filter(|d| d.has_disc()) {
        let same = now.iter().find(|d| d.id == old.id).is_some_and(|d| {
            d.has_disc() && d.media == old.media && d.tracks == old.tracks && d.length_ms == old.length_ms
        });
        if !same {
            events.push(DiscEvent::Removed { drive: old.clone() });
        }
    }
    for new in now.iter().filter(|d| d.has_disc()) {
        let known = before.get(&new.id).is_some_and(|d| {
            d.has_disc() && d.media == new.media && d.tracks == new.tracks && d.length_ms == new.length_ms
        });
        if !known {
            events.push(DiscEvent::Inserted { drive: new.clone() });
        }
    }
    events
}

/// Looks at the drives every `watch_seconds` while "Tell me when a disc is inserted" is on and
/// no rip is running, and tells the screen about discs going in and out (`disc` event). What
/// is already in a drive at startup is the baseline: no notice for it.
pub async fn watch(app: AppHandle) {
    tokio::time::sleep(WATCH_START_DELAY).await;
    loop {
        let settings = app.state::<AppState>().settings().disc;
        tokio::time::sleep(Duration::from_secs(u64::from(settings.watch_seconds))).await;
        let disc = app.state::<Disc>();
        if !settings.watch || disc.ripping() {
            continue;
        }
        let lock = disc.drive.clone();
        let listed = tokio::task::spawn_blocking(move || {
            // a job that has the drive (an open, a rip) knows what is in it: don't disturb it
            let _one_user = lock.try_lock().ok()?;
            homefi_disc::list_drives().ok()
        })
        .await;
        let Ok(Some(drives)) = listed else { continue };
        let now: Vec<DriveView> = drives.iter().map(DriveView::of).collect();
        disc.remember_names(&now);
        let events = {
            let mut seen = locked(&disc.seen);
            let events = seen.as_ref().map(|before| changes(before, &now)).unwrap_or_default();
            *seen = Some(now.into_iter().map(|d| (d.id.clone(), d)).collect());
            events
        };
        for event in events {
            let _ = app.emit("disc", event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn info(media: Media) -> DriveInfo {
        DriveInfo { id: DriveId("disk4".into()), name: "HL-DT-ST BD-RE".into(), media }
    }

    fn view(media: Media) -> DriveView {
        DriveView::of(&info(media))
    }

    fn json(view: &DriveView) -> serde_json::Value {
        serde_json::to_value(view).unwrap()
    }

    #[test]
    fn media_maps_to_what_the_screen_expects() {
        assert_eq!(json(&view(Media::None)), serde_json::json!({"id":"disk4","name":"HL-DT-ST BD-RE","media":"none"}));
        assert_eq!(
            json(&view(Media::AudioCd { tracks: 12, length_ms: 3_000_000 })),
            serde_json::json!({"id":"disk4","name":"HL-DT-ST BD-RE","media":"audio","tracks":12,"lengthMs":3_000_000})
        );
        assert_eq!(json(&view(Media::Protected(ProtectionKind::DvdCss)))["protection"], "css");
        assert_eq!(json(&view(Media::Protected(ProtectionKind::DvdCppm)))["protection"], "cppm");
        assert_eq!(json(&view(Media::Protected(ProtectionKind::BluRayAacs)))["protection"], "aacs");
        let data = json(&view(Media::DataDisc { mount: Some(PathBuf::from("/Volumes/PHOTOS")) }));
        assert_eq!((data["media"].as_str(), data["mount"].as_str()), (Some("data"), Some("/Volumes/PHOTOS")));
        assert_eq!(json(&view(Media::Unknown))["media"], "unknown");
    }

    #[test]
    fn disc_events_have_the_shapes_of_api_ts() {
        let drive = view(Media::AudioCd { tracks: 2, length_ms: 1000 });
        let inserted = serde_json::to_value(DiscEvent::Inserted { drive: drive.clone() }).unwrap();
        assert_eq!(inserted["type"], "inserted");
        assert_eq!(inserted["drive"]["id"], "disk4");
        let isrcs = serde_json::to_value(DiscEvent::Isrcs { drive, isrcs: vec![Some("A".into()), None] }).unwrap();
        assert_eq!(isrcs["type"], "isrcs");
        assert_eq!(isrcs["isrcs"], serde_json::json!(["A", null]));
    }

    #[test]
    fn the_rip_request_from_the_screen_reads() {
        let args: RipArgs = serde_json::from_str(
            r#"{"drive":"disk4","tracks":[1,3],"format":{"kind":"mp3","mp3":"v0"},"coverFile":null,
                "meta":{"source":"cdText","confidence":1,
                  "album":{"title":"Korn","artist":"Korn","year":1994,"genre":null,"upc":null,"discNumber":1,"discTotal":1,"tidalAlbumId":null,"mbid":null},
                  "tracks":[{"number":1,"title":"Blind","artists":["Korn"],"isrc":null,"tidalTrackId":null}],
                  "cover":null,"note":null}}"#,
        )
        .unwrap();
        assert_eq!(args.tracks, [1, 3]);
        assert_eq!(args.meta.album.year, Some(1994));
        assert!(matches!(args.format, RipFormat::Mp3 { .. }));
    }

    fn drives(list: &[DriveView]) -> HashMap<String, DriveView> {
        list.iter().map(|d| (d.id.clone(), d.clone())).collect()
    }

    fn one(view: &DriveView) -> HashMap<String, DriveView> {
        drives(std::slice::from_ref(view))
    }

    #[test]
    fn a_disc_going_in_and_out_is_noticed_once() {
        let empty = view(Media::None);
        let cd = view(Media::AudioCd { tracks: 12, length_ms: 3_000_000 });
        assert!(changes(&one(&empty), std::slice::from_ref(&empty)).is_empty());
        assert_eq!(changes(&one(&empty), std::slice::from_ref(&cd)), [DiscEvent::Inserted { drive: cd.clone() }]);
        assert!(changes(&one(&cd), std::slice::from_ref(&cd)).is_empty(), "still there: no second notice");
        assert_eq!(changes(&one(&cd), std::slice::from_ref(&empty)), [DiscEvent::Removed { drive: cd.clone() }]);
        assert_eq!(changes(&one(&cd), &[]), [DiscEvent::Removed { drive: cd }], "unplugged drive");
    }

    #[test]
    fn swapping_discs_between_two_looks_sends_out_then_in() {
        let first = view(Media::AudioCd { tracks: 12, length_ms: 3_000_000 });
        let other = view(Media::AudioCd { tracks: 9, length_ms: 2_000_000 });
        let events = changes(&one(&first), std::slice::from_ref(&other));
        assert_eq!(events, [DiscEvent::Removed { drive: first }, DiscEvent::Inserted { drive: other }]);
    }

    #[test]
    fn a_drive_that_cannot_say_what_it_holds_is_not_news() {
        let unknown = view(Media::Unknown);
        let empty = view(Media::None);
        assert!(changes(&drives(&[empty]), &[unknown]).is_empty());
    }

    #[test]
    fn the_first_audio_disc_is_picked_when_no_drive_is_named() {
        let drives = vec![
            DriveInfo { id: DriveId("a".into()), name: "A".into(), media: Media::None },
            DriveInfo { id: DriveId("b".into()), name: "B".into(), media: Media::AudioCd { tracks: 1, length_ms: 1 } },
        ];
        assert_eq!(pick_drive(drives.clone(), None).unwrap().id.0, "b");
        assert_eq!(pick_drive(drives.clone(), Some("a")).unwrap().id.0, "a");
        assert!(matches!(pick_drive(drives, Some("zzz")), Err(DiscError::NoDrive)));
        assert!(matches!(pick_drive(Vec::new(), None), Err(DiscError::NoDrive)));
    }

    #[test]
    fn only_one_rip_runs_and_the_cancel_reaches_it() {
        let disc = Disc::new();
        let first = disc.begin().unwrap();
        assert!(disc.begin().is_err());
        assert!(disc.ripping());
        disc.cancel();
        assert!(first.load(Ordering::Relaxed));
        disc.finish(&first);
        assert!(!disc.ripping());
        assert!(disc.begin().is_ok());
    }

    #[tokio::test]
    async fn cover_files_must_be_pictures() {
        let dir = tempfile::tempdir().unwrap();
        let jpeg = dir.path().join("a.jpg");
        std::fs::write(&jpeg, [0xFF, 0xD8, 0xFF, 0xD9]).unwrap();
        let text_file = dir.path().join("a.txt");
        std::fs::write(&text_file, b"hello").unwrap();
        assert_eq!(read_cover_file(&jpeg).await.unwrap(), vec![0xFF, 0xD8, 0xFF, 0xD9]);
        assert!(read_cover_file(&text_file).await.is_err());
        assert!(read_cover_file(&dir.path().join("missing.jpg")).await.is_err());
    }
}
