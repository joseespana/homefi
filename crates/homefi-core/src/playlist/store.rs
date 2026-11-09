//! The playlists folder: one JSON file per playlist.

use std::path::{Path, PathBuf};

use super::model::{HomePlaylist, PLAYLISTS_DIR};
use crate::error::{Error, Result};

/// Longest playlist id.
const MAX_ID_LEN: usize = 64;
const EXTENSION: &str = "json";

/// Ids come from the UI, so they are checked before they touch a path: that
/// is what stops path traversal.
fn valid_id(id: &str) -> bool {
    (1..=MAX_ID_LEN).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

#[derive(Debug, Clone)]
pub struct PlaylistStore {
    dir: PathBuf,
}

impl PlaylistStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The store inside the app folder (`<app dir>/playlists`).
    pub fn in_app_dir(app_dir: &Path) -> Self {
        Self::new(app_dir.join(PLAYLISTS_DIR))
    }

    fn path_of(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.{EXTENSION}"))
    }

    /// Every readable playlist, most recently updated first. Files that don't
    /// parse are skipped (and kept); no folder gives an empty list.
    pub fn list(&self) -> Vec<HomePlaylist> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut playlists: Vec<HomePlaylist> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == EXTENSION))
            .filter(|p| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
            .filter_map(|path| {
                let read = std::fs::read(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|bytes| serde_json::from_slice::<HomePlaylist>(&bytes).map_err(|e| e.to_string()));
                match read {
                    Ok(playlist) => Some(playlist),
                    Err(error) => {
                        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        tracing::warn!("skipping playlist file {name}: {error}");
                        None
                    }
                }
            })
            .collect();
        playlists.sort_by(|a, b| b.updated_ms.cmp(&a.updated_ms).then_with(|| a.id.cmp(&b.id)));
        playlists
    }

    pub fn load(&self, id: &str) -> Result<HomePlaylist> {
        if !valid_id(id) {
            return Err(Error::Io(std::io::ErrorKind::NotFound.into()));
        }
        Ok(serde_json::from_slice(&std::fs::read(self.path_of(id))?)?)
    }

    /// Writes `playlist`. A copy that doesn't know its TIDAL twin (one a screen held from
    /// before it was saved there) keeps the `tidal_uuid` on disk, so the next save to TIDAL
    /// updates that copy instead of making a second one.
    pub fn save(&self, playlist: &HomePlaylist) -> Result<()> {
        if !valid_id(&playlist.id) {
            return Err(Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid playlist id")));
        }
        let known_uuid =
            if playlist.tidal_uuid.is_none() { self.load(&playlist.id).ok().and_then(|p| p.tidal_uuid) } else { None };
        let bytes = match known_uuid {
            Some(uuid) => serde_json::to_vec_pretty(&HomePlaylist { tidal_uuid: Some(uuid), ..playlist.clone() })?,
            None => serde_json::to_vec_pretty(playlist)?,
        };
        crate::paths::write_atomic(&self.path_of(&playlist.id), &bytes)?;
        Ok(())
    }

    /// Records (or forgets, with `None`) the TIDAL copy of a stored playlist, leaving the rest
    /// as it is on disk now. `false` when the playlist isn't stored anymore.
    pub fn set_tidal_uuid(&self, id: &str, uuid: Option<String>) -> Result<bool> {
        let Ok(mut stored) = self.load(id) else {
            return Ok(false);
        };
        if stored.tidal_uuid == uuid {
            return Ok(true);
        }
        stored.tidal_uuid = uuid;
        let bytes = serde_json::to_vec_pretty(&stored)?;
        crate::paths::write_atomic(&self.path_of(id), &bytes)?;
        Ok(true)
    }

    /// The file of an existing playlist (so a desktop can move it to the Trash).
    pub fn file_path(&self, id: &str) -> Option<PathBuf> {
        if !valid_id(id) {
            return None;
        }
        let path = self.path_of(id);
        path.is_file().then_some(path)
    }

    /// Deletes HomeFi's own JSON for `id`; `false` when there is none. Never
    /// touches library files.
    pub fn remove(&self, id: &str) -> Result<bool> {
        let Some(path) = self.file_path(id) else {
            return Ok(false);
        };
        std::fs::remove_file(path)?;
        Ok(true)
    }
}
