//! Local music library: scans folders for playable audio files and reads
//! their tags and technical properties (format, sample rate, bit depth).
//!
//! Used by the Android player; results are cached by file size + mtime so a
//! rescan only parses new or changed files.

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use lofty::config::ParseOptions;
use lofty::file::{AudioFile, FileType, TaggedFileExt};
use lofty::picture::PictureType;
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::{ItemKey, Tag};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Extensions the app's player (Media3 / Android decoders) can play.
pub const PLAYABLE_EXTENSIONS: &[&str] =
    &["flac", "mp3", "wav", "m4a", "mp4", "aac", "ogg", "oga", "opus", "mka", "amr"];

/// Folder images used when a file has no embedded cover.
const FOLDER_COVERS: &[&str] = &["cover", "folder", "front", "album"];

/// A playable file with its tags and audio properties.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackInfo {
    pub path: PathBuf,
    /// Tag title, or the file name without extension.
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub track_number: u32,
    pub disc_number: u32,
    pub year: Option<u32>,
    pub genre: String,
    pub duration_ms: u64,
    /// `FLAC`, `MP3`, `WAV`, `AAC`, `ALAC`, `Vorbis`, `Opus`, ...
    pub format: String,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u8>,
    pub channels: Option<u8>,
    pub bitrate_kbps: Option<u32>,
    pub lossless: bool,
    pub size_bytes: u64,
    /// International Standard Recording Code, links the file to Tidal.
    #[serde(default)]
    pub isrc: Option<String>,
}

impl TrackInfo {
    /// `FLAC · 24-bit / 96 kHz`, `MP3 · 320 kbps`, ...
    pub fn quality_label(&self) -> String {
        let mut parts = vec![self.format.clone()];
        if self.lossless {
            match (self.bit_depth, self.sample_rate) {
                (Some(bits), Some(rate)) => parts.push(format!("{bits}-bit / {} kHz", khz(rate))),
                (None, Some(rate)) => parts.push(format!("{} kHz", khz(rate))),
                _ => {}
            }
        } else if let Some(kbps) = self.bitrate_kbps {
            parts.push(format!("{kbps} kbps"));
        }
        parts.join(" · ")
    }
}

fn khz(rate: u32) -> String {
    let khz = rate as f64 / 1000.0;
    if khz.fract() == 0.0 {
        format!("{khz:.0}")
    } else {
        format!("{khz:.1}")
    }
}

/// `true` when the extension is one the player supports.
pub fn is_playable(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| PLAYABLE_EXTENSIONS.iter().any(|p| p.eq_ignore_ascii_case(e)))
}

fn file_type_name(file_type: FileType) -> &'static str {
    match file_type {
        FileType::Aac => "AAC",
        FileType::Aiff => "AIFF",
        FileType::Ape => "APE",
        FileType::Flac => "FLAC",
        FileType::Mpeg => "MP3",
        FileType::Mp4 => "MP4",
        FileType::Mpc => "Musepack",
        FileType::Opus => "Opus",
        FileType::Vorbis => "Vorbis",
        FileType::Speex => "Speex",
        FileType::Wav => "WAV",
        FileType::WavPack => "WavPack",
        _ => "Audio",
    }
}

/// Codec inside an MP4 container (`AAC`, `ALAC`, `FLAC`, ...).
fn mp4_codec(path: &Path) -> Option<&'static str> {
    use lofty::mp4::{Mp4Codec, Mp4File};
    let mut file = File::open(path).ok()?;
    let mp4 = Mp4File::read_from(&mut file, ParseOptions::new().read_tags(false).read_cover_art(false)).ok()?;
    Some(match mp4.properties().codec() {
        Mp4Codec::AAC => "AAC",
        Mp4Codec::ALAC => "ALAC",
        Mp4Codec::MP3 => "MP3",
        Mp4Codec::FLAC => "FLAC",
        _ => "MP4",
    })
}

fn tag_year(tag: &Tag) -> Option<u32> {
    [ItemKey::Year, ItemKey::RecordingDate, ItemKey::OriginalReleaseDate]
        .iter()
        .filter_map(|k| tag.get_string(k))
        .find_map(|s| s.get(..4).and_then(|y| y.parse().ok()))
}

/// Reads tags and properties of one file (cover art is not loaded).
pub fn read_track(path: &Path) -> Result<TrackInfo> {
    let meta_err = |e: &dyn std::fmt::Display| Error::Metadata { path: path.to_path_buf(), message: e.to_string() };

    let tagged = Probe::open(path)
        .map_err(|e| meta_err(&e))?
        .options(ParseOptions::new().read_cover_art(false))
        .guess_file_type()
        .map_err(|e| meta_err(&e))?
        .read()
        .map_err(|e| meta_err(&e))?;

    let props = tagged.properties();
    let format = match tagged.file_type() {
        FileType::Mp4 => mp4_codec(path).unwrap_or("MP4"),
        other => file_type_name(other),
    };
    let lossless = matches!(format, "FLAC" | "ALAC" | "WAV" | "AIFF" | "APE" | "WavPack");

    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    let text = |f: fn(&Tag) -> Option<std::borrow::Cow<'_, str>>| {
        tag.and_then(f).map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
    };

    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let artist = text(|t| t.artist()).unwrap_or_default();
    let album_artist = tag.and_then(|t| t.get_string(&ItemKey::AlbumArtist)).map(str::to_owned);

    Ok(TrackInfo {
        path: path.to_path_buf(),
        title: text(|t| t.title()).unwrap_or(stem),
        album_artist: album_artist.filter(|a| !a.trim().is_empty()).unwrap_or_else(|| artist.clone()),
        artist,
        album: text(|t| t.album()).unwrap_or_default(),
        track_number: tag.and_then(|t| t.track()).unwrap_or(0),
        disc_number: tag.and_then(|t| t.disk()).unwrap_or(0),
        year: tag.and_then(tag_year),
        genre: text(|t| t.genre()).unwrap_or_default(),
        duration_ms: props.duration().as_millis() as u64,
        format: format.to_owned(),
        sample_rate: props.sample_rate(),
        // lossy codecs report a nominal bit depth that means nothing to listeners
        bit_depth: props.bit_depth().filter(|_| lossless),
        channels: props.channels(),
        bitrate_kbps: props.audio_bitrate().or(props.overall_bitrate()).filter(|b| *b > 0),
        lossless,
        size_bytes: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
        isrc: tag.and_then(|t| t.get_string(&ItemKey::Isrc)).map(|s| s.trim().to_uppercase()).filter(|s| !s.is_empty()),
    })
}

/// Minimal entry for a playable file whose tags can't be parsed (e.g. Matroska):
/// it still shows up in the library and the player decodes it.
pub fn basic_track(path: &Path) -> TrackInfo {
    let ext = path.extension().map(|e| e.to_string_lossy().to_uppercase()).unwrap_or_default();
    TrackInfo {
        path: path.to_path_buf(),
        title: path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
        artist: String::new(),
        album: String::new(),
        album_artist: String::new(),
        track_number: 0,
        disc_number: 0,
        year: None,
        genre: String::new(),
        duration_ms: 0,
        lossless: matches!(ext.as_str(), "FLAC" | "WAV"),
        format: ext,
        sample_rate: None,
        bit_depth: None,
        channels: None,
        bitrate_kbps: None,
        size_bytes: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
        isrc: None,
    }
}

/// Embedded front cover (or any embedded picture), else a folder image
/// such as `cover.jpg` next to the file.
pub fn read_cover(path: &Path) -> Option<Vec<u8>> {
    if let Ok(tagged) = lofty::read_from_path(path) {
        let pictures: Vec<_> = tagged.tags().iter().flat_map(|t| t.pictures()).collect();
        let front = pictures.iter().find(|p| p.pic_type() == PictureType::CoverFront).or(pictures.first());
        if let Some(picture) = front {
            return Some(picture.data().to_vec());
        }
    }

    let dir = path.parent()?;
    let mut candidates: Vec<(usize, PathBuf)> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let ext = p.extension()?.to_str()?.to_ascii_lowercase();
            let stem = p.file_stem()?.to_str()?.to_ascii_lowercase();
            if !matches!(ext.as_str(), "jpg" | "jpeg" | "png") {
                return None;
            }
            FOLDER_COVERS.iter().position(|c| *c == stem).map(|rank| (rank, p))
        })
        .collect();
    candidates.sort();
    candidates.first().and_then(|(_, p)| std::fs::read(p).ok())
}

#[derive(Clone, Serialize, Deserialize)]
struct CacheEntry {
    size: u64,
    modified: i64,
    info: TrackInfo,
}

#[derive(Default, Serialize, Deserialize)]
struct Cache {
    version: u32,
    entries: HashMap<PathBuf, CacheEntry>,
}

const CACHE_VERSION: u32 = 2;

fn load_cache(path: Option<&Path>) -> Cache {
    path.and_then(|p| std::fs::read(p).ok())
        .and_then(|raw| serde_json::from_slice::<Cache>(&raw).ok())
        .filter(|c| c.version == CACHE_VERSION)
        .unwrap_or_default()
}

fn modified_secs(meta: &std::fs::Metadata) -> i64 {
    meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Drops roots that do not exist or are inside another root.
fn normalize_roots(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut canonical: Vec<PathBuf> =
        roots.iter().filter_map(|r| r.canonicalize().ok()).filter(|r| r.is_dir()).collect();
    canonical.sort();
    canonical.dedup();
    let mut out: Vec<PathBuf> = Vec::new();
    for root in canonical {
        if !out.iter().any(|kept| root.starts_with(kept)) {
            out.push(root);
        }
    }
    out
}

/// Playable files below `dir` (hidden files and folders are skipped,
/// symlinks are not followed).
fn collect_files(dir: &Path, out: &mut Vec<(PathBuf, std::fs::Metadata)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let Ok(file_type) = entry.file_type() else { continue };
        let path = entry.path();
        if file_type.is_dir() {
            collect_files(&path, out);
        } else if file_type.is_file() && is_playable(&path) {
            if let Ok(meta) = entry.metadata() {
                out.push((path, meta));
            }
        }
    }
}

/// Scans `roots` recursively. With `cache`, unchanged files are not parsed
/// again and the cache file is rewritten with the current state.
pub fn scan(roots: &[PathBuf], cache: Option<&Path>) -> Vec<TrackInfo> {
    let mut files = Vec::new();
    for root in normalize_roots(roots) {
        collect_files(&root, &mut files);
    }

    let mut old = load_cache(cache);
    let mut tracks = Vec::with_capacity(files.len());
    let mut pending = Vec::new();

    for (path, meta) in files {
        let (size, modified) = (meta.len(), modified_secs(&meta));
        match old.entries.remove(&path) {
            Some(entry) if entry.size == size && entry.modified == modified => tracks.push(entry),
            _ => pending.push((path, size, modified)),
        }
    }

    // parse new / changed files in parallel
    let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).min(8);
    let chunk = pending.len().div_ceil(workers).max(1);
    let parsed: Vec<CacheEntry> = std::thread::scope(|s| {
        let handles: Vec<_> = pending
            .chunks(chunk)
            .map(|part| {
                s.spawn(move || {
                    part.iter()
                        .map(|(path, size, modified)| {
                            let info = read_track(path).unwrap_or_else(|e| {
                                tracing::debug!("no tags for {}: {e}", path.display());
                                basic_track(path)
                            });
                            CacheEntry { size: *size, modified: *modified, info }
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    });
    tracks.extend(parsed);

    tracks.sort_by(|a, b| {
        // untagged files go last
        let key = |t: &CacheEntry| {
            let i = &t.info;
            (
                i.album_artist.is_empty(),
                i.album_artist.to_lowercase(),
                i.album.to_lowercase(),
                i.disc_number,
                i.track_number,
            )
        };
        key(a).cmp(&key(b)).then_with(|| a.info.title.to_lowercase().cmp(&b.info.title.to_lowercase()))
    });

    if let Some(cache_path) = cache {
        let new_cache = Cache {
            version: CACHE_VERSION,
            entries: tracks.iter().map(|e| (e.info.path.clone(), e.clone())).collect(),
        };
        let write = serde_json::to_vec(&new_cache).map_err(std::io::Error::from).and_then(|raw| {
            if let Some(parent) = cache_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(cache_path, raw)
        });
        if let Err(e) = write {
            tracing::warn!("could not write library cache {}: {e}", cache_path.display());
        }
    }

    tracks.into_iter().map(|e| e.info).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playable_extensions() {
        assert!(is_playable(Path::new("a/b.FLAC")));
        assert!(is_playable(Path::new("x.opus")));
        assert!(!is_playable(Path::new("cover.jpg")));
        assert!(!is_playable(Path::new("noext")));
    }

    #[test]
    fn nested_roots_are_merged() {
        let dir = tempfile::tempdir().unwrap();
        let music = dir.path().join("Music");
        std::fs::create_dir_all(music.join("HomeFi")).unwrap();
        let roots = normalize_roots(&[music.join("HomeFi"), music.clone(), dir.path().join("missing")]);
        assert_eq!(roots, vec![music.canonicalize().unwrap()]);
    }

    #[test]
    fn quality_labels() {
        let mut t = TrackInfo {
            path: "a.flac".into(),
            title: "t".into(),
            artist: String::new(),
            album: String::new(),
            album_artist: String::new(),
            track_number: 0,
            disc_number: 0,
            year: None,
            genre: String::new(),
            duration_ms: 0,
            format: "FLAC".into(),
            sample_rate: Some(96_000),
            bit_depth: Some(24),
            channels: Some(2),
            bitrate_kbps: Some(3000),
            lossless: true,
            size_bytes: 0,
            isrc: None,
        };
        assert_eq!(t.quality_label(), "FLAC · 24-bit / 96 kHz");
        t.sample_rate = Some(44_100);
        t.bit_depth = Some(16);
        assert_eq!(t.quality_label(), "FLAC · 16-bit / 44.1 kHz");
        t.format = "MP3".into();
        t.lossless = false;
        t.bitrate_kbps = Some(320);
        assert_eq!(t.quality_label(), "MP3 · 320 kbps");
    }

    #[test]
    fn folder_cover_fallback() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Folder.JPG"), b"folder").unwrap();
        std::fs::write(dir.path().join("cover.jpg"), b"cover").unwrap();
        std::fs::write(dir.path().join("song.flac"), b"not really flac").unwrap();
        assert_eq!(read_cover(&dir.path().join("song.flac")).as_deref(), Some(&b"cover"[..]));
    }
}
