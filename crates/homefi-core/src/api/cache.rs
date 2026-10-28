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

    /// The cached body for `key` if younger than `ttl`. Blocking: async code
    /// uses [`Self::get_async`].
    pub fn get(&self, key: &str, ttl: Duration) -> Option<String> {
        let raw = std::fs::read(self.file(key)).ok()?;
        let entry: Entry = serde_json::from_slice(&raw).ok()?;
        (now_secs() - entry.stored_at < ttl.as_secs() as i64).then_some(entry.body)
    }

    /// Stores `body` for `key`. The file is replaced atomically (a temporary
    /// file, then a rename), so concurrent readers never see half an entry.
    /// Blocking: async code uses [`Self::put_async`].
    pub fn put(&self, key: &str, body: &str) {
        let file = self.file(key);
        let write = || -> std::io::Result<()> {
            if !self.dir.exists() {
                std::fs::create_dir_all(&self.dir)?;
                set_mode(&self.dir, 0o700);
            }
            let entry = Entry { stored_at: now_secs(), body: body.to_owned() };
            // temporary files are created 0600
            crate::paths::write_atomic(&file, &serde_json::to_vec(&entry)?)
        };
        if let Err(e) = write() {
            tracing::debug!("cache write failed for {}: {e}", file.display());
        }
    }

    /// [`Self::get`] on the blocking pool, so the async workers never wait on the disk.
    pub async fn get_async(&self, key: &str, ttl: Duration) -> Option<String> {
        let (cache, key) = (self.clone(), key.to_owned());
        tokio::task::spawn_blocking(move || cache.get(&key, ttl)).await.ok().flatten()
    }

    /// [`Self::put`] on the blocking pool.
    pub async fn put_async(&self, key: &str, body: String) {
        let (cache, key) = (self.clone(), key.to_owned());
        let _ = tokio::task::spawn_blocking(move || cache.put(&key, &body)).await;
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

    #[tokio::test]
    async fn async_access_and_atomic_concurrent_writes() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().join("c"));
        cache.put_async("k", "{}".into()).await;
        assert_eq!(cache.get_async("k", Duration::from_secs(60)).await.as_deref(), Some("{}"));

        // many writers of big bodies: every read sees one whole body, never a mix
        let bodies: Vec<String> = (0..8).map(|i| format!("{{\"v\":\"{}\"}}", i.to_string().repeat(50_000))).collect();
        let writers: Vec<_> = bodies
            .iter()
            .cloned()
            .map(|b| {
                let c = cache.clone();
                tokio::spawn(async move { c.put_async("big", b).await })
            })
            .collect();
        for _ in 0..20 {
            if let Some(read) = cache.get_async("big", Duration::from_secs(60)).await {
                assert!(bodies.contains(&read), "torn cache entry");
            }
        }
        for w in writers {
            w.await.unwrap();
        }
        let read = cache.get_async("big", Duration::from_secs(60)).await.unwrap();
        assert!(bodies.contains(&read));
        // no temporary files left behind
        let leftovers = walk(&dir.path().join("c")).into_iter().filter(|p| !p.ends_with(".json")).count();
        assert_eq!(leftovers, 0);
    }

    fn walk(dir: &std::path::Path) -> Vec<String> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            if e.path().is_dir() {
                out.extend(walk(&e.path()));
            } else {
                out.push(e.path().to_string_lossy().into_owned());
            }
        }
        out
    }
}
