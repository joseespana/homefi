//! Ripping: the drive thread reads a track (secure reader), the encoder thread writes it, a
//! bounded pipe of recycled chunks joins them. After each track the file is tagged like a
//! download, named with the downloads template and moved into place.
//!
//! One track at a time, in the order asked. A track that fails does not stop the others; a
//! drive or disc that is gone, or a cancel, does. Files already saved stay.

use std::cell::Cell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use homefi_core::humane;
use homefi_core::metadata::{write_track_tags, TrackTags};
use homefi_core::models::{Album, MediaItem, Track};
use homefi_core::template::{clean_segment, format_path, TemplateData};
use serde::Serialize;
use tokio::runtime::Handle;

use crate::chunks::{chunk_channel, DEFAULT_CHUNKS};
use crate::cue::{self, Cue, CueTrack};
use crate::encode::{self, EncodeJob, RipFormat, Stage, Verified};
use crate::error::{DiscError, Result};
use crate::identify::{DiscMatch, TrackMeta};
use crate::log::{self, RipLog, TrackLog, TrackOutcome};
use crate::read::{read_track, ReadPlan, ReadProgress, TrackRead};
use crate::settings::{DiscSettings, OnUnreadable};
use crate::source::SectorSource;
use crate::toc::Toc;

/// Reading progress goes out at most this often (the rest is dropped, not queued).
const PROGRESS_EVERY: Duration = Duration::from_millis(100);
/// Work files in the album folder: no dots, so ffmpeg's suffix handling can't trip on them.
const WORK_PREFIX: &str = "homefi-rip-";
const YEAR_ONLY_MONTH_DAY: &str = "01-01";

/// The settings a rip honours besides the format and the reading plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RipOptions {
    pub flac_verify: bool,
    pub on_unreadable: OnUnreadable,
    pub write_log: bool,
    pub write_cue: bool,
    /// Also write `cover.jpg` / `cover.png` into the album folder.
    pub save_cover_file: bool,
}

impl RipOptions {
    pub fn from_settings(settings: &DiscSettings) -> Self {
        RipOptions {
            flac_verify: settings.flac_verify,
            on_unreadable: settings.on_unreadable,
            write_log: settings.write_log,
            write_cue: settings.write_cue,
            save_cover_file: settings.cover.save_file,
        }
    }
}

/// Where the files go and how they are named.
#[derive(Debug, Clone)]
pub struct RipOutput {
    /// The download folder (or the folder chosen for rips).
    pub dir: PathBuf,
    /// A downloads-style template (`{album.artist}/{album.title}/{item.title}`).
    pub template: String,
    /// Put the cover inside the files (the app's "embed cover" setting).
    pub embed_cover: bool,
    /// The cover's bytes (a JPEG or PNG), if there is one.
    pub cover: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct RipRequest {
    /// Name of the drive, for the log.
    pub drive_name: String,
    /// Track numbers, in the order to rip.
    pub tracks: Vec<u8>,
    /// What the user decided; blanks are already filled in by the screen.
    pub meta: DiscMatch,
    pub format: RipFormat,
    pub plan: ReadPlan,
    pub options: RipOptions,
    pub out: RipOutput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RipProgress {
    Started { track: u8, title: String },
    Reading { track: u8, done_sectors: u32, total_sectors: u32, rereads: u32, suspicious_samples: u64 },
    Encoding { track: u8, percent: u8 },
    Verifying { track: u8 },
    TrackDone { track: u8, path: PathBuf, read: TrackRead, verified: Option<Verified> },
    TrackFailed { track: u8, message: String },
    Finished { ok: u32, failed: u32, log_path: Option<PathBuf> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RipSummary {
    pub ok: u32,
    pub failed: u32,
    pub log_path: Option<PathBuf>,
    /// The folder the album was written to.
    pub album_dir: Option<PathBuf>,
}

/// A track to rip and the file it will become.
struct Planned {
    number: u8,
    title: String,
    isrc: Option<String>,
    artists: Vec<String>,
    path: PathBuf,
}

struct Saved {
    path: PathBuf,
    read: TrackRead,
    crc32: u32,
    verified: Option<Verified>,
}

/// `YYYY-01-01`: the tag writers want a full date, a disc only knows the year.
fn date_of(year: Option<u16>) -> Option<String> {
    year.map(|y| format!("{y:04}-{YEAR_ONLY_MONTH_DAY}"))
}

fn artist_json(name: &str) -> serde_json::Value {
    serde_json::json!({ "id": 0, "name": name, "type": "MAIN" })
}

/// The disc as the models the downloads template understands (`id: 0`: not a Tidal item).
fn template_models(meta: &DiscMatch, track: &TrackMeta) -> Result<(Track, Album)> {
    let album = &meta.album;
    let album_json = serde_json::json!({
        "id": 0,
        "title": album.title,
        "artist": artist_json(&album.artist),
        "artists": [artist_json(&album.artist)],
        "releaseDate": date_of(album.year),
        "type": "ALBUM",
        "numberOfVolumes": album.disc_total,
    });
    let artists = track_artists(meta, track);
    let track_json = serde_json::json!({
        "id": 0,
        "title": track.title,
        "trackNumber": track.number,
        "volumeNumber": album.disc_number,
        "isrc": track.isrc,
        "artist": artists.first().map(|a| artist_json(a)),
        "artists": artists.iter().map(|a| artist_json(a)).collect::<Vec<_>>(),
        "album": { "title": album.title },
    });
    let parse = |error: serde_json::Error| DiscError::Encode(format!("track description: {error}"));
    Ok((serde_json::from_value(track_json).map_err(parse)?, serde_json::from_value(album_json).map_err(parse)?))
}

fn track_artists(meta: &DiscMatch, track: &TrackMeta) -> Vec<String> {
    if track.artists.is_empty() {
        vec![meta.album.artist.clone()]
    } else {
        track.artists.clone()
    }
}

fn track_meta<'a>(meta: &'a DiscMatch, number: u8) -> std::borrow::Cow<'a, TrackMeta> {
    match meta.tracks.iter().find(|t| t.number == number) {
        Some(track) => std::borrow::Cow::Borrowed(track),
        None => {
            std::borrow::Cow::Owned(TrackMeta { number, title: format!("Track {number:02}"), ..TrackMeta::default() })
        }
    }
}

/// Where every requested track goes. Two tracks with the same name get " (2)", " (3)"...
fn plan_files(req: &RipRequest, toc: &Toc) -> Result<Vec<Planned>> {
    let ext = req.format.extension();
    let mut used: HashSet<PathBuf> = HashSet::new();
    let mut planned = Vec::with_capacity(req.tracks.len());
    for &number in &req.tracks {
        let entry = toc.track(number).ok_or(DiscError::NoSuchTrack(number))?;
        if !entry.audio {
            return Err(DiscError::NotAudioTrack(number));
        }
        let meta = track_meta(&req.meta, number);
        let (track, album) = template_models(&req.meta, &meta)?;
        let data = TemplateData::new("").item(&MediaItem::Track(track)).album(&album);
        let relative = format_path(&req.out.template, &data, false)?;
        let mut path = req.out.dir.join(format!("{relative}.{ext}"));
        let mut copy = 2;
        while !used.insert(path.clone()) {
            path = req.out.dir.join(format!("{relative} ({copy}).{ext}"));
            copy += 1;
        }
        planned.push(Planned {
            number,
            title: meta.title.clone(),
            isrc: meta.isrc.clone(),
            artists: track_artists(&req.meta, &meta),
            path,
        });
    }
    Ok(planned)
}

fn tags_for(req: &RipRequest, track: &Planned) -> TrackTags {
    let album = &req.meta.album;
    TrackTags {
        title: track.title.clone(),
        track_number: i64::from(track.number),
        disc_number: i64::from(album.disc_number),
        album_artist: album.artist.clone(),
        artists: track.artists.clone(),
        album_title: album.title.clone(),
        date: date_of(album.year),
        isrc: track.isrc.clone(),
        genre: album.genre.clone().unwrap_or_default(),
        cover: req.out.embed_cover.then(|| req.out.cover.clone()).flatten(),
        ..TrackTags::default()
    }
}

/// Reads, encodes, tags and files one track.
#[allow(clippy::too_many_arguments)]
fn rip_track(
    src: &mut dyn SectorSource,
    toc: &Toc,
    req: &RipRequest,
    track: &Planned,
    work_dir: &Path,
    cancel: &AtomicBool,
    runtime: &Handle,
    progress: &(dyn Fn(RipProgress) + Sync),
) -> Result<Saved> {
    let number = track.number;
    let stem = work_dir.join(format!("{WORK_PREFIX}{number:02}"));
    let (mut producer, consumer) = chunk_channel(DEFAULT_CHUNKS);
    let on_stage = |stage: Stage| {
        progress(match stage {
            Stage::Finishing => RipProgress::Encoding { track: number, percent: 100 },
            Stage::Verifying => RipProgress::Verifying { track: number },
        })
    };
    let job = EncodeJob {
        format: req.format,
        verify_flac: req.options.flac_verify,
        stem: &stem,
        frames: toc.track_frames(number).unwrap_or(0),
        runtime,
        on_stage: &on_stage,
    };
    let last_report = Cell::new(None::<Instant>);
    let report = |p: ReadProgress| {
        let due = last_report.get().is_none_or(|at| at.elapsed() >= PROGRESS_EVERY);
        if due || p.done_sectors == p.total_sectors {
            last_report.set(Some(Instant::now()));
            progress(RipProgress::Reading {
                track: number,
                done_sectors: p.done_sectors,
                total_sectors: p.total_sectors,
                rereads: p.rereads,
                suspicious_samples: p.suspicious_samples,
            });
        }
    };

    let (read, encoded) = std::thread::scope(|scope| {
        let job = &job;
        let encoder = std::thread::Builder::new()
            .name("disc-encode".into())
            .spawn_scoped(scope, move || encode::encode_track(job, &consumer))?;
        let read = {
            let mut sink = producer.sink(cancel);
            read_track(src, toc, number, &req.plan, &mut sink, &report)
        };
        // the encoder sees the end of the audio when the producer is gone
        drop(producer);
        let encoded =
            encoder.join().unwrap_or_else(|_| Err(DiscError::Encode("the encoder stopped unexpectedly".into())));
        Ok::<_, DiscError>((read, encoded))
    })?;

    let discard = |path: &Path| {
        let _ = std::fs::remove_file(path);
    };
    if cancel.load(Ordering::Relaxed) {
        if let Ok(done) = &encoded {
            discard(&done.path);
        }
        return Err(DiscError::Cancelled);
    }
    let (read, encoded) = match (read, encoded) {
        // the reader stopped because the encoder did: the encoder's error is the cause
        (Err(DiscError::Cancelled), Err(error)) | (Ok(_), Err(error)) => return Err(error),
        (Err(error), done) => {
            if let Ok(done) = &done {
                discard(&done.path);
            }
            return Err(error);
        }
        (Ok(read), Ok(encoded)) => (read, encoded),
    };
    if read.unreadable_samples > 0 && req.options.on_unreadable == OnUnreadable::Skip {
        discard(&encoded.path);
        return Err(DiscError::Unreadable { track: number, samples: read.unreadable_samples });
    }

    if let Err(error) = write_track_tags(&encoded.path, &tags_for(req, track)) {
        // the audio is good: keep the file, say so in the log
        tracing::warn!(track = number, %error, "could not tag the ripped file");
    }
    if let Some(parent) = track.path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if let Err(error) = std::fs::rename(&encoded.path, &track.path) {
        discard(&encoded.path);
        return Err(error.into());
    }
    Ok(Saved { path: track.path.clone(), read, crc32: encoded.digest.crc32(), verified: encoded.verified })
}

fn failure_text(error: &DiscError) -> String {
    tracing::debug!(%error, "track not ripped");
    error.humane(humane::language())
}

/// Rips `req.tracks` from `src`. `cancel` stops it at the next chunk (`Err(Cancelled)`); the
/// tracks saved before stay. Blocking: run it on its own thread.
pub fn rip(
    src: &mut dyn SectorSource,
    req: &RipRequest,
    cancel: &AtomicBool,
    runtime: &Handle,
    progress: &(dyn Fn(RipProgress) + Sync),
) -> Result<RipSummary> {
    encode::ensure_available(&req.format)?;
    let toc = src.toc()?;
    let planned = plan_files(req, &toc)?;
    let Some(first) = planned.first() else {
        return Ok(RipSummary { ok: 0, failed: 0, log_path: None, album_dir: None });
    };
    let album_dir = first.path.parent().unwrap_or(&req.out.dir).to_path_buf();
    std::fs::create_dir_all(&album_dir)?;

    let mut logs: Vec<TrackLog> = Vec::with_capacity(planned.len());
    let (mut ok, mut failed) = (0u32, 0u32);
    let mut stopped: Option<DiscError> = None;
    for track in &planned {
        if cancel.load(Ordering::Relaxed) {
            stopped = Some(DiscError::Cancelled);
            break;
        }
        progress(RipProgress::Started { track: track.number, title: track.title.clone() });
        match rip_track(src, &toc, req, track, &album_dir, cancel, runtime, progress) {
            Ok(saved) => {
                ok += 1;
                let file = saved.path.strip_prefix(&album_dir).unwrap_or(&saved.path).to_string_lossy().into_owned();
                logs.push(TrackLog {
                    number: track.number,
                    title: track.title.clone(),
                    outcome: TrackOutcome::Saved {
                        file,
                        read: saved.read,
                        crc32: saved.crc32,
                        verified: saved.verified,
                    },
                });
                progress(RipProgress::TrackDone {
                    track: track.number,
                    path: saved.path,
                    read: saved.read,
                    verified: saved.verified,
                });
            }
            Err(DiscError::Cancelled) => {
                stopped = Some(DiscError::Cancelled);
                break;
            }
            Err(error) => {
                failed += 1;
                let message = failure_text(&error);
                logs.push(TrackLog {
                    number: track.number,
                    title: track.title.clone(),
                    outcome: TrackOutcome::Failed(message.clone()),
                });
                progress(RipProgress::TrackFailed { track: track.number, message });
                if error.is_fatal() {
                    stopped = Some(error);
                    break;
                }
            }
        }
    }

    let log_path = write_sheets(req, &planned, &logs, &album_dir);
    progress(RipProgress::Finished { ok, failed, log_path: log_path.clone() });
    match stopped {
        Some(error) => Err(error),
        None => Ok(RipSummary { ok, failed, log_path, album_dir: Some(album_dir) }),
    }
}

/// The log, the cue sheet and the cover file, as asked. Failing to write them never fails a rip.
fn write_sheets(req: &RipRequest, planned: &[Planned], logs: &[TrackLog], album_dir: &Path) -> Option<PathBuf> {
    let album = &req.meta.album;
    let base = clean_segment(&album.title);
    let write = |name: String, text: String| -> Option<PathBuf> {
        let path = album_dir.join(name);
        std::fs::write(&path, text)
            .inspect_err(|error| tracing::warn!(%error, "could not write {}", path.display()))
            .ok()?;
        Some(path)
    };

    if req.options.save_cover_file {
        if let Some(cover) = &req.out.cover {
            let ext = if cover.starts_with(&[0x89, b'P', b'N', b'G']) { "png" } else { "jpg" };
            let path = album_dir.join(format!("cover.{ext}"));
            if !path.exists() {
                let _ = std::fs::write(&path, cover)
                    .inspect_err(|error| tracing::warn!(%error, "could not save the cover"));
            }
        }
    }
    if req.options.write_cue {
        let saved: Vec<(&Planned, String)> = planned
            .iter()
            .filter(|p| logs.iter().any(|l| l.number == p.number && matches!(l.outcome, TrackOutcome::Saved { .. })))
            .map(|p| (p, p.path.strip_prefix(album_dir).unwrap_or(&p.path).to_string_lossy().into_owned()))
            .collect();
        let tracks: Vec<CueTrack<'_>> = saved
            .iter()
            .map(|(p, file)| CueTrack {
                number: p.number,
                title: &p.title,
                artist: p.artists.first().map_or(album.artist.as_str(), String::as_str),
                isrc: p.isrc.as_deref(),
                file,
            })
            .collect();
        let sheet = cue::render(&Cue {
            album: &album.title,
            artist: &album.artist,
            genre: album.genre.as_deref(),
            year: album.year,
            catalog: album.upc.as_deref(),
            format: &req.format,
            tracks: &tracks,
        });
        write(format!("{base}.cue"), sheet);
    }
    if req.options.write_log {
        let when = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
        let text = log::render(&RipLog {
            app_version: env!("CARGO_PKG_VERSION"),
            when: &when,
            drive: &req.drive_name,
            artist: &album.artist,
            album: &album.title,
            format: &req.format,
            plan: &req.plan,
            tracks: logs,
        });
        return write(format!("{base}.homefi-rip.log"), text);
    }
    None
}
