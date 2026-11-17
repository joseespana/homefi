//! Tiny on-disk HTTP response cache.
//!
//! Entries are keyed by the request URL (never the bearer token) and stored
//! as one JSON file each, readable only by the current user.
//!
//! The cache is regenerable (Tidal answers again) and lives in its own folder
//! (`<app dir>/api_cache`), apart from the user's downloads and library: nothing
//! here ever looks outside that folder. It keeps to a disk budget
//! ([`DiskCache::set_max_mb`], 0 = off): after some writes a background prune
//! drops entries no request can use any more ([`MAX_AGE`]), then the oldest ones
//! until the folder fits again. Reads never wait for a prune (no locks; a file
//! deleted under a reader is just a miss).

use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::auth::{now_secs, set_mode};

/// The disk budget when nothing says otherwise.
pub const DEFAULT_MAX_MB: u32 = 200;
/// The biggest budget a setting may ask for.
pub const MAX_MAX_MB: u32 = 4096;
/// The longest any response is served from the cache (the longest `CachePolicy`
/// TTL): an older entry can never be read again, so a prune deletes it.
pub const MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);

const BYTES_PER_MB: u64 = 1024 * 1024;
/// A response bigger than this is never stored (it would push out many small ones).
const MAX_ENTRY_BYTES: u64 = 4 * BYTES_PER_MB;
/// …nor one bigger than this share of the budget (1/8), for small budgets.
const ENTRY_BUDGET_DIVISOR: u64 = 8;
/// A prune runs at most this often while responses keep coming…
const PRUNE_INTERVAL_SECS: i64 = 5 * 60;
/// …or sooner, once this share of the budget (1/10) was written since the last one.
const PRUNE_WRITTEN_DIVISOR: u64 = 10;
/// A prune that has to drop entries leaves the folder at this share of the
/// budget, so the next few writes don't start another one at once.
const PRUNE_TARGET_PERCENT: u64 = 90;
/// Temporary files of a write that died (a crash) are deleted after this long.
const STALE_TEMP: Duration = Duration::from_secs(3600);
/// Prefix and suffix of [`crate::paths::write_atomic`]'s temporary files.
const TEMP_PREFIX: &str = ".homefi-";
const TEMP_SUFFIX: &str = ".tmp";
const ENTRY_SUFFIX: &str = ".json";

/// The budget and the prune bookkeeping, shared by every clone of a cache.
#[derive(Debug)]
struct Budget {
    max_bytes: AtomicU64,
    /// Bytes stored since the last prune.
    written: AtomicU64,
    /// When the last prune started (Unix seconds; 0 = never, so the first write prunes).
    last_prune_secs: AtomicI64,
    /// One prune at a time.
    pruning: AtomicBool,
}

/// Holds the right to prune; releases it when dropped (even on a panic).
struct PruneGuard(Arc<Budget>);

impl PruneGuard {
    fn claim(budget: &Arc<Budget>) -> Option<Self> {
        budget
            .pruning
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
            .then(|| Self(Arc::clone(budget)))
    }
}

impl Drop for PruneGuard {
    fn drop(&mut self) {
        self.0.pruning.store(false, Ordering::Release);
    }
}

/// The on-disk response cache. Clones share the folder and the budget.
#[derive(Debug, Clone)]
pub struct DiskCache {
    dir: PathBuf,
    budget: Arc<Budget>,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    stored_at: i64,
    body: String,
}

fn mb_to_bytes(mb: u32) -> u64 {
    u64::from(mb.min(MAX_MAX_MB)) * BYTES_PER_MB
}

fn is_temp(name: &str) -> bool {
    name.starts_with(TEMP_PREFIX) && name.ends_with(TEMP_SUFFIX)
}

impl DiskCache {
    /// A cache in `dir` with the [`DEFAULT_MAX_MB`] budget.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            budget: Arc::new(Budget {
                max_bytes: AtomicU64::new(mb_to_bytes(DEFAULT_MAX_MB)),
                written: AtomicU64::new(0),
                last_prune_secs: AtomicI64::new(0),
                pruning: AtomicBool::new(false),
            }),
        }
    }

    /// [`Self::new`] with a budget of `mb` (see [`Self::set_max_mb`]).
    pub fn with_max_mb(self, mb: u32) -> Self {
        self.set_max_mb(mb);
        self
    }

    /// The disk budget in MB (at most [`MAX_MAX_MB`]); 0 turns the cache off:
    /// nothing is read or stored, and the next prune deletes what is there.
    /// Every clone sees the change. Call [`Self::prune_async`] after a smaller
    /// budget to make room now rather than after the next write.
    pub fn set_max_mb(&self, mb: u32) {
        self.budget.max_bytes.store(mb_to_bytes(mb), Ordering::Relaxed);
        // the next write prunes against the new budget
        self.budget.last_prune_secs.store(0, Ordering::Relaxed);
    }

    /// The disk budget in bytes (0 = off).
    pub fn max_bytes(&self) -> u64 {
        self.budget.max_bytes.load(Ordering::Relaxed)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn file(&self, key: &str) -> PathBuf {
        let hash = Sha256::digest(key.as_bytes());
        let name: String = hash.iter().map(|b| format!("{b:02x}")).collect();
        self.dir.join(&name[..2]).join(format!("{name}{ENTRY_SUFFIX}"))
    }

    /// Whether a body of `len` bytes may be stored: the cache is on and the body
    /// is not bigger than [`MAX_ENTRY_BYTES`] or 1/8 of the budget.
    fn accepts(&self, len: usize) -> bool {
        let max = self.max_bytes();
        max > 0 && (len as u64) <= MAX_ENTRY_BYTES.min(max / ENTRY_BUDGET_DIVISOR)
    }

    /// The cached body for `key` if younger than `ttl`. Blocking: async code
    /// uses [`Self::get_async`].
    pub fn get(&self, key: &str, ttl: Duration) -> Option<String> {
        if self.max_bytes() == 0 {
            return None;
        }
        let raw = std::fs::read(self.file(key)).ok()?;
        let entry: Entry = serde_json::from_slice(&raw).ok()?;
        (now_secs() - entry.stored_at < ttl.as_secs() as i64).then_some(entry.body)
    }

    /// Writes the entry; `true` when it was stored.
    fn store(&self, key: &str, body: &str) -> bool {
        if !self.accepts(body.len()) {
            return false;
        }
        let file = self.file(key);
        let write = || -> std::io::Result<usize> {
            if !self.dir.exists() {
                std::fs::create_dir_all(&self.dir)?;
                set_mode(&self.dir, 0o700);
            }
            let entry = Entry { stored_at: now_secs(), body: body.to_owned() };
            let bytes = serde_json::to_vec(&entry)?;
            // temporary files are created 0600
            crate::paths::write_atomic(&file, &bytes)?;
            Ok(bytes.len())
        };
        match write() {
            Ok(len) => {
                self.budget.written.fetch_add(len as u64, Ordering::Relaxed);
                true
            }
            Err(e) => {
                tracing::debug!("cache write failed for {}: {e}", file.display());
                false
            }
        }
    }

    /// Whether enough was written, or enough time went by, for another prune.
    fn prune_due(&self) -> bool {
        let written = self.budget.written.load(Ordering::Relaxed);
        let since_secs = now_secs() - self.budget.last_prune_secs.load(Ordering::Relaxed);
        written >= self.max_bytes() / PRUNE_WRITTEN_DIVISOR || since_secs >= PRUNE_INTERVAL_SECS
    }

    /// Stores `body` for `key` (skipped when the cache is off or the body is too
    /// big). The file is replaced atomically (a temporary file, then a rename),
    /// so concurrent readers never see half an entry. Prunes here when one is due.
    /// Blocking: async code uses [`Self::put_async`].
    pub fn put(&self, key: &str, body: &str) {
        if self.store(key, body) && self.prune_due() {
            if let Some(guard) = PruneGuard::claim(&self.budget) {
                self.prune_claimed(guard);
            }
        }
    }

    /// [`Self::get`] on the blocking pool, so the async workers never wait on the disk.
    pub async fn get_async(&self, key: &str, ttl: Duration) -> Option<String> {
        if self.max_bytes() == 0 {
            return None;
        }
        let (cache, key) = (self.clone(), key.to_owned());
        tokio::task::spawn_blocking(move || cache.get(&key, ttl)).await.ok().flatten()
    }

    /// [`Self::put`] on the blocking pool. A due prune runs on its own blocking
    /// task afterwards, which this does not wait for.
    pub async fn put_async(&self, key: &str, body: String) {
        if !self.accepts(body.len()) {
            return;
        }
        let (cache, key) = (self.clone(), key.to_owned());
        let stored = tokio::task::spawn_blocking(move || cache.store(&key, &body)).await.unwrap_or(false);
        if stored && self.prune_due() {
            if let Some(guard) = PruneGuard::claim(&self.budget) {
                let cache = self.clone();
                // detached: the request that wrote the entry returns now
                drop(tokio::task::spawn_blocking(move || cache.prune_claimed(guard)));
            }
        }
    }

    /// Deletes entries no request can use any more, stale temporary files, then
    /// the oldest entries (by modification time) while the folder is over its
    /// budget. Returns the bytes freed; 0 at once when another prune is running.
    /// Blocking: async code uses [`Self::prune_async`].
    pub fn prune(&self) -> u64 {
        PruneGuard::claim(&self.budget).map_or(0, |guard| self.prune_claimed(guard))
    }

    /// [`Self::prune`] on the blocking pool.
    pub async fn prune_async(&self) -> u64 {
        let cache = self.clone();
        tokio::task::spawn_blocking(move || cache.prune()).await.unwrap_or(0)
    }

    fn prune_claimed(&self, _guard: PruneGuard) -> u64 {
        self.budget.written.store(0, Ordering::Relaxed);
        self.budget.last_prune_secs.store(now_secs(), Ordering::Relaxed);
        let max_bytes = self.max_bytes();
        let now = SystemTime::now();
        let mut freed: u64 = 0;
        let mut kept: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
        self.for_each_file(|path, meta| {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let age = now.duration_since(modified).unwrap_or(Duration::ZERO);
            let expired = if is_temp(name) {
                // a write in progress is young: only a crash leaves an old one behind
                age > STALE_TEMP
            } else if name.ends_with(ENTRY_SUFFIX) {
                age > MAX_AGE
            } else {
                return; // not ours: leave it
            };
            if expired {
                if std::fs::remove_file(path).is_ok() {
                    freed += meta.len();
                }
            } else if !is_temp(name) {
                kept.push((modified, meta.len(), path.to_path_buf()));
            }
        });

        let mut total: u64 = kept.iter().map(|(_, len, _)| len).sum();
        if total > max_bytes {
            let target = max_bytes / 100 * PRUNE_TARGET_PERCENT;
            kept.sort_unstable_by_key(|(modified, _, _)| *modified);
            for (_, len, path) in &kept {
                if total <= target {
                    break;
                }
                // a reader holding it open still reads it whole; a later one misses
                if std::fs::remove_file(path).is_ok() {
                    total -= len;
                    freed += len;
                }
            }
        }
        if freed > 0 {
            tracing::debug!("api cache: freed {} KB, {} KB left", freed / 1024, total / 1024);
        }
        freed
    }

    /// Calls `f` for every file in the cache folder and its subfolders (one level, as [`Self::file`] lays them out).
    fn for_each_file(&self, mut f: impl FnMut(&Path, &Metadata)) {
        let Ok(top) = std::fs::read_dir(&self.dir) else { return };
        for sub in top.flatten() {
            let Ok(kind) = sub.file_type() else { continue };
            if !kind.is_dir() {
                continue;
            }
            let Ok(files) = std::fs::read_dir(sub.path()) else { continue };
            for file in files.flatten() {
                if let Ok(meta) = file.metadata() {
                    if meta.is_file() {
                        f(&file.path(), &meta);
                    }
                }
            }
        }
    }

    /// Bytes the cache takes on disk now. Blocking: async code uses [`Self::usage_async`].
    pub fn usage_bytes(&self) -> u64 {
        let mut total = 0;
        self.for_each_file(|_, meta| total += meta.len());
        total
    }

    /// [`Self::usage_bytes`] on the blocking pool.
    pub async fn usage_async(&self) -> u64 {
        let cache = self.clone();
        tokio::task::spawn_blocking(move || cache.usage_bytes()).await.unwrap_or(0)
    }

    /// Removes every cached response (only this cache's own folder).
    pub fn clear(&self) -> std::io::Result<()> {
        self.budget.written.store(0, Ordering::Relaxed);
        match std::fs::remove_dir_all(&self.dir) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    /// [`Self::clear`] on the blocking pool.
    pub async fn clear_async(&self) -> std::io::Result<()> {
        let cache = self.clone();
        tokio::task::spawn_blocking(move || cache.clear()).await.map_err(std::io::Error::other)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: Duration = Duration::from_secs(3600);

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

    #[test]
    fn many_keys_stay_within_the_budget() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().join("c")).with_max_mb(1);
        let budget = cache.max_bytes();
        let body = "x".repeat(10 * 1024);
        // ~10 MB written into a 1 MB budget
        for i in 0..1000 {
            cache.put(&format!("key-{i}"), &body);
        }
        // pruning runs along the way: never more than a tenth over (what's written between two prunes)
        assert!(cache.usage_bytes() <= budget + budget / PRUNE_WRITTEN_DIVISOR + 16 * 1024, "{}", cache.usage_bytes());
        cache.prune();
        assert!(cache.usage_bytes() <= budget);
        // the newest entries are the ones kept
        assert!(cache.get("key-999", HOUR).is_some());
        assert!(cache.get("key-0", HOUR).is_none());
    }

    #[test]
    fn oversize_bodies_are_not_cached() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().join("c"));
        let huge = "x".repeat(MAX_ENTRY_BYTES as usize + 1);
        cache.put("huge", &huge);
        assert!(cache.get("huge", HOUR).is_none());
        assert_eq!(cache.usage_bytes(), 0);

        // a small budget takes at most an eighth of itself per entry
        let small = DiskCache::new(dir.path().join("s")).with_max_mb(1);
        small.put("big", &"y".repeat((BYTES_PER_MB / ENTRY_BUDGET_DIVISOR) as usize + 1));
        assert!(small.get("big", HOUR).is_none());
        small.put("ok", "{}");
        assert_eq!(small.get("ok", HOUR).as_deref(), Some("{}"));
    }

    #[test]
    fn off_reads_and_stores_nothing_and_prunes_everything() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().join("c"));
        cache.put("k", "{}");
        let other = cache.clone();
        other.set_max_mb(0);
        assert!(cache.get("k", HOUR).is_none(), "clones share the budget");
        cache.put("k2", "{}");
        cache.prune();
        assert_eq!(cache.usage_bytes(), 0);
        cache.set_max_mb(DEFAULT_MAX_MB);
        cache.put("k3", "{}");
        assert!(cache.get("k3", HOUR).is_some());
    }

    #[test]
    fn prune_drops_old_entries_and_crash_leftovers_only() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().join("c"));
        cache.put("old", "{}");
        cache.put("new", "{}");
        let long_ago = SystemTime::now() - MAX_AGE - Duration::from_secs(60);
        let set_mtime =
            |p: &Path| std::fs::File::options().write(true).open(p).unwrap().set_modified(long_ago).unwrap();
        set_mtime(&cache.file("old"));
        let sub = cache.file("new").parent().unwrap().to_path_buf();
        let stale_tmp = sub.join(".homefi-abc.tmp");
        let fresh_tmp = sub.join(".homefi-def.tmp");
        let foreign = sub.join("notes.txt");
        for p in [&stale_tmp, &fresh_tmp, &foreign] {
            std::fs::write(p, "x").unwrap();
        }
        set_mtime(&stale_tmp);
        set_mtime(&foreign);

        cache.prune();
        assert!(!cache.file("old").exists());
        assert!(cache.file("new").exists());
        assert!(!stale_tmp.exists() && fresh_tmp.exists(), "a write in progress is left alone");
        assert!(foreign.exists(), "files that aren't the cache's are never touched");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn pruning_never_breaks_concurrent_reads() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().join("c")).with_max_mb(1);
        let body = format!("{{\"v\":\"{}\"}}", "z".repeat(20 * 1024));
        cache.put_async("hot", body.clone()).await;

        let writer = {
            let (cache, body) = (cache.clone(), body.clone());
            tokio::spawn(async move {
                for i in 0..300 {
                    cache.put_async(&format!("w-{i}"), body.clone()).await;
                }
            })
        };
        let pruner = {
            let cache = cache.clone();
            tokio::spawn(async move {
                for _ in 0..50 {
                    cache.prune_async().await;
                }
            })
        };
        // a read is a whole body or a miss: never an error, never a torn entry
        let mut reads = 0;
        while !writer.is_finished() || !pruner.is_finished() {
            if let Some(read) = cache.get_async("hot", HOUR).await {
                assert_eq!(read, body);
            }
            reads += 1;
        }
        writer.await.unwrap();
        pruner.await.unwrap();
        assert!(reads > 0);
        // a detached prune started by the last writes may still be walking: let it finish, then prune once more
        while cache.budget.pruning.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        cache.prune_async().await;
        assert!(cache.usage_bytes() <= cache.max_bytes());
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
