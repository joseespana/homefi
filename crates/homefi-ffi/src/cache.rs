//! The saved Tidal answers (the API response cache) for the apps' settings:
//! its disk budget, how much it takes now and a way to empty it. A thin bridge
//! over `homefi_core::api::DiskCache`; downloads and the library are never touched.

use std::sync::Arc;

use crate::{HomefiEngine, Result};

#[uniffi::export]
impl HomefiEngine {
    /// Disk space the saved Tidal answers may take, in MB (at most 4096; 0 keeps
    /// none). Returns once what no longer fits is deleted (off the main thread).
    pub async fn set_api_cache_mb(self: Arc<Self>, mb: u32) {
        crate::runtime::run(async move {
            let cache = self.session.cache();
            cache.set_max_mb(mb);
            cache.prune_async().await;
        })
        .await;
    }

    /// Bytes the saved Tidal answers take on disk now.
    pub async fn api_cache_usage(self: Arc<Self>) -> u64 {
        crate::runtime::run(async move { self.session.cache().usage_async().await }).await
    }

    /// Deletes every saved Tidal answer (they come back as Tidal answers again).
    pub async fn clear_api_cache(self: Arc<Self>) -> Result<()> {
        crate::runtime::run(async move {
            self.session.cache().clear_async().await.map_err(|e| homefi_core::Error::from(e).into())
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_usage_and_clear() {
        let dir = tempfile::tempdir().unwrap();
        let engine = HomefiEngine::new(dir.path().to_string_lossy().into_owned()).unwrap();
        let cache = engine.session.cache().clone();
        cache.put("k", "{}");
        let used = crate::runtime::RT.block_on(Arc::clone(&engine).api_cache_usage());
        assert!(used > 0);
        crate::runtime::RT.block_on(Arc::clone(&engine).clear_api_cache()).unwrap();
        assert_eq!(crate::runtime::RT.block_on(Arc::clone(&engine).api_cache_usage()), 0);
        cache.put("k", "{}");
        crate::runtime::RT.block_on(Arc::clone(&engine).set_api_cache_mb(0));
        assert_eq!(cache.max_bytes(), 0);
        assert_eq!(cache.usage_bytes(), 0, "off deletes what was saved");
    }
}
