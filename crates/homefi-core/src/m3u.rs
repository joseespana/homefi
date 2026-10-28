//! `.m3u` playlist files.

use std::fmt::Write as _;
use std::path::{Component, Path, PathBuf};

use crate::error::Result;
use crate::models::Track;
use crate::paths::{relative_to, with_suffix};

/// Writes `tracks` to `<path>.m3u` with paths relative to the playlist file.
/// Returns the playlist path, or `None` when there was nothing to write.
pub fn save(tracks: &[(PathBuf, Track)], path: &Path) -> Result<Option<PathBuf>> {
    if tracks.is_empty() {
        tracing::warn!("not writing {}: no tracks", path.display());
        return Ok(None);
    }

    let file = with_suffix(path, "m3u");
    let dir = file.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;

    let mut out = String::from("#EXTM3U\n");
    for (track_path, track) in tracks {
        let artist = track.artist.as_ref().map(|a| a.name.as_str()).unwrap_or_default();
        let relative = relative_to(track_path, dir);
        let _ = writeln!(out, "#EXTINF:{},{artist} - {}", track.duration, track.title);
        let _ = writeln!(out, "{}", relative.to_string_lossy().replace('\\', "/"));
    }
    std::fs::write(&file, out)?;
    Ok(Some(file))
}

/// A playlist file read from disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalPlaylist {
    /// File name without the extension.
    pub name: String,
    pub path: PathBuf,
    /// Absolute paths of the entries, in order (they may not exist).
    pub tracks: Vec<PathBuf>,
}

/// Reads an `.m3u` / `.m3u8` file. Relative entries are resolved against the
/// playlist's folder; `#` lines, blank lines, a BOM, `\r\n` and URLs are
/// tolerated (URLs are skipped).
pub fn read(path: &Path) -> Result<LocalPlaylist> {
    let raw = std::fs::read(path)?;
    let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let dir = path.parent().unwrap_or(Path::new("."));
    let dir = if dir.is_absolute() { dir.to_path_buf() } else { std::env::current_dir()?.join(dir) };
    Ok(LocalPlaylist { name, path: path.to_path_buf(), tracks: parse(&String::from_utf8_lossy(&raw), &dir) })
}

/// Entries of playlist text `text`, resolved against `dir`.
pub fn parse(text: &str, dir: &Path) -> Vec<PathBuf> {
    text.trim_start_matches('\u{feff}')
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|line| {
            let line = match line.strip_prefix("file://") {
                Some(rest) => rest,
                None if line.contains("://") => return None,
                None => line,
            };
            // playlists made on Windows use backslashes
            let entry = PathBuf::from(line.replace('\\', "/"));
            Some(normalize(&if entry.is_absolute() { entry } else { dir.join(entry) }))
        })
        .collect()
}

/// Removes `.` and `..` components without touching the file system, so
/// entries compare equal to the paths a library scan reports.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push(c);
                }
            }
            c => out.push(c),
        }
    }
    out
}

/// Finds `.m3u` / `.m3u8` files below `roots` (hidden folders are skipped),
/// sorted by name. Unreadable files are left out.
pub fn find(roots: &[PathBuf]) -> Vec<LocalPlaylist> {
    let mut files = Vec::new();
    for root in roots {
        collect(root, &mut files, 0);
    }
    files.sort();
    files.dedup();
    let mut lists: Vec<LocalPlaylist> =
        files.iter().filter_map(|f| read(f).map_err(|e| tracing::debug!("{}: {e}", f.display())).ok()).collect();
    lists.sort_by_cached_key(|l| (l.name.to_lowercase(), l.path.clone()));
    lists
}

/// Folder depth limit, against pathological trees.
const MAX_DEPTH: usize = 32;

fn collect(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        // `file_type` does not follow symlinks, so linked folders can't loop
        let Ok(kind) = entry.file_type() else { continue };
        let path = entry.path();
        if kind.is_dir() {
            if depth < MAX_DEPTH {
                collect(&path, out, depth + 1);
            }
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("m3u") || e.eq_ignore_ascii_case("m3u8"))
        {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_relative_absolute_and_windows_entries() {
        let text = "\u{feff}#EXTM3U\r\n\r\n#EXTINF:296,Queen - Somebody To Love\r\n../Queen/Album/Song.flac\r\n\
                    ./Local.mp3\n/abs/Other.flac\nhttp://radio/stream\nfile:///abs/Uri.flac\n..\\Win\\Song.flac\n";
        let tracks = parse(text, Path::new("/sd/m3u"));
        assert_eq!(
            tracks,
            ["/sd/Queen/Album/Song.flac", "/sd/m3u/Local.mp3", "/abs/Other.flac", "/abs/Uri.flac", "/sd/Win/Song.flac"]
                .map(PathBuf::from)
        );
    }

    #[test]
    fn finds_playlists_and_skips_hidden_folders() {
        let root = tempfile::tempdir().unwrap();
        let sd = root.path();
        std::fs::create_dir_all(sd.join("m3u/album")).unwrap();
        std::fs::create_dir_all(sd.join(".trash")).unwrap();
        std::fs::write(
            sd.join("m3u/Road Trip.m3u"),
            "#EXTM3U\r\n#EXTINF:296,Queen - Somebody To Love\r\n../Queen/A/Song.flac\r\n",
        )
        .unwrap();
        std::fs::write(sd.join("m3u/album/b.M3U8"), "x.flac\n").unwrap();
        std::fs::write(sd.join(".trash/old.m3u"), "x.flac\n").unwrap();
        std::fs::write(sd.join("m3u/notes.txt"), "x.flac\n").unwrap();

        let lists = find(&[sd.to_path_buf(), sd.join("m3u")]);
        let names: Vec<&str> = lists.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["b", "Road Trip"]);
        assert_eq!(lists[1].tracks, [sd.join("Queen/A/Song.flac")]);
        assert_eq!(lists[0].tracks, [sd.join("m3u/album/x.flac")]);
        assert_eq!(read(&sd.join("m3u/Road Trip.m3u")).unwrap(), lists[1]);
        assert!(read(&sd.join("missing.m3u")).is_err());
    }

    #[test]
    fn writes_relative_entries() {
        let dir = tempfile::tempdir().unwrap();
        let track: Track = serde_json::from_value(serde_json::json!({
            "id": 1, "title": "Song", "duration": 200, "artist": {"id": 1, "name": "Band", "type": "MAIN"}
        }))
        .unwrap();
        let tracks = vec![(dir.path().join("Band/Album/Song.flac"), track)];
        let file = save(&tracks, &dir.path().join("m3u/album/Band - Album.*")).unwrap().unwrap();
        let text = std::fs::read_to_string(file).unwrap();
        assert_eq!(text, "#EXTM3U\n#EXTINF:200,Band - Song\n../../Band/Album/Song.flac\n");
    }
}
