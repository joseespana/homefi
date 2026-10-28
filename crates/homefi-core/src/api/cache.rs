//! Tiny on-disk HTTP response cache.
//!
//! Entries are keyed by the request URL (never the bearer token) and stored
//! as one JSON file each, readable only by the current user.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::auth::{now_secs, set_mode};

#[derive(Debug, Clone)]
pub struct DiskCache {
    dir: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    stored_at: i64,
    body: String,
}

impl DiskCache {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn file(&self, key: &str) -> PathBuf {
        let hash = Sha256::digest(key.as_bytes());
        let name: String = hash.iter().map(|b| format!("{b:02x}")).collect();
        self.dir.join(&name[..2]).join(format!("{name}.json"))
    }

    pub fn get(&self, key: &str, ttl: Duration) -> Option<String> {
        let raw = std::fs::read(self.file(key)).ok()?;
        let entry: Entry = serde_json::from_slice(&raw).ok()?;
        (now_secs() - entry.stored_at < ttl.as_secs() as i64).then_some(entry.body)
    }

    pub fn put(&self, key: &str, body: &str) {
        let file = self.file(key);
        let write = || -> std::io::Result<()> {
            let parent = file.parent().expect("cache file has a parent");
            if !self.dir.exists() {
                std::fs::create_dir_all(&self.dir)?;
                set_mode(&self.dir, 0o700);
            }
            std::fs::create_dir_all(parent)?;
            let entry = Entry { stored_at: now_secs(), body: body.to_owned() };
            std::fs::write(&file, serde_json::to_vec(&entry)?)?;
            set_mode(&file, 0o600);
            Ok(())
        };
        if let Err(e) = write() {
            tracing::debug!("cache write failed for {}: {e}", file.display());
        }
    }

    /// Removes every cached response.
    pub fn clear(&self) -> std::io::Result<()> {
        match std::fs::remove_dir_all(&self.dir) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_expiry() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().join("c"));
        assert!(cache.get("k", Duration::from_secs(60)).is_none());
        cache.put("k", "{\"a\":1}");
        assert_eq!(cache.get("k", Duration::from_secs(60)).as_deref(), Some("{\"a\":1}"));
        assert!(cache.get("k", Duration::ZERO).is_none());
        cache.clear().unwrap();
        assert!(cache.get("k", Duration::from_secs(60)).is_none());
    }
}
