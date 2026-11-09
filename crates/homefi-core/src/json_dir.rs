//! A folder with one JSON file per item (`<dir>/<id>.json`): atomic writes, lenient listing. The
//! persistence pattern of the playlist store, shared by the saved rides and the DJ setups.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::{Error, Result};

/// Longest id.
const MAX_ID_LEN: usize = 64;
const EXTENSION: &str = "json";
/// Low bits of the per-process counter that keeps ids made in the same millisecond apart.
const ID_COUNTER_MASK: u32 = 0xfff;

/// Ids come from the UI, so they are checked before they touch a path: that is what stops path traversal.
pub fn valid_id(id: &str) -> bool {
    (1..=MAX_ID_LEN).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// A new id: `prefix`, the time in ms and a 12-bit counter, in hex.
pub fn new_id(prefix: &str) -> String {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    format!("{prefix}{:x}{:x}", crate::playlist::now_ms(), COUNTER.fetch_add(1, Ordering::Relaxed) & ID_COUNTER_MASK)
}

#[derive(Debug, Clone)]
pub struct JsonDir {
    dir: PathBuf,
}

impl JsonDir {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn path(&self) -> &Path {
        &self.dir
    }

    fn path_of(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.{EXTENSION}"))
    }

    /// Every readable item, in no particular order. A file that doesn't parse is skipped (and kept);
    /// no folder gives an empty list.
    pub fn list<T: DeserializeOwned>(&self) -> Vec<T> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == EXTENSION))
            .filter(|p| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
            .filter_map(|path| {
                let read = std::fs::read(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|bytes| serde_json::from_slice::<T>(&bytes).map_err(|e| e.to_string()));
                match read {
                    Ok(item) => Some(item),
                    Err(error) => {
                        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        tracing::warn!("skipping file {name}: {error}");
                        None
                    }
                }
            })
            .collect()
    }

    pub fn load<T: DeserializeOwned>(&self, id: &str) -> Result<T> {
        if !valid_id(id) {
            return Err(Error::Io(std::io::ErrorKind::NotFound.into()));
        }
        Ok(serde_json::from_slice(&std::fs::read(self.path_of(id))?)?)
    }

    pub fn save<T: Serialize>(&self, id: &str, item: &T) -> Result<()> {
        if !valid_id(id) {
            return Err(Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid id")));
        }
        crate::paths::write_atomic(&self.path_of(id), &serde_json::to_vec_pretty(item)?)?;
        Ok(())
    }

    /// Deletes the file of `id`; `false` when there is none. These are the app's own data files.
    pub fn remove(&self, id: &str) -> Result<bool> {
        if !valid_id(id) {
            return Ok(false);
        }
        let path = self.path_of(id);
        if !path.is_file() {
            return Ok(false);
        }
        std::fs::remove_file(path)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_checked_and_unique() {
        assert!(valid_id("r1a-b_C") && !valid_id("") && !valid_id("../x") && !valid_id(&"a".repeat(65)));
        assert_ne!(new_id("r"), new_id("r"));
        assert!(valid_id(&new_id("d")));
    }

    #[test]
    fn a_bad_file_is_skipped_not_fatal() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = JsonDir::new(tmp.path().join("nested"));
        assert!(dir.list::<Vec<u8>>().is_empty(), "no folder");
        dir.save("a", &vec![1u8, 2]).unwrap();
        std::fs::write(tmp.path().join("nested/b.json"), "not json").unwrap();
        assert_eq!(dir.list::<Vec<u8>>(), vec![vec![1u8, 2]]);
        assert!(dir.load::<Vec<u8>>("../a").is_err());
        assert!(dir.remove("a").unwrap() && !dir.remove("a").unwrap());
    }
}
