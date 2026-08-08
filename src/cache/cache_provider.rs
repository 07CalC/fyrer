use std::{path::PathBuf, sync::Arc};

use anyhow::Result;

use crate::{
    cache::{CacheMetadata, local::LocalCacheProvider},
    config::CacheProviderKind,
};

/// Pluggable interface for reading and writing the task output cache.
///
/// Implementations must be `Send + Sync` so they can be wrapped in an
/// [`Arc`] and shared across async tasks without restriction.
pub trait CacheProvider: Send + Sync {
    /// Returns `true` if the cache entry for `key` exists.
    fn contains(&self, key: &str) -> bool;

    /// Attempts to restore a cached entry. Returns `true` on success.
    ///
    /// A return value of `false` (not an error) means the entry was not
    /// found or was corrupt; the caller should fall through to execution.
    fn restore(&self, key: &str, output_hash: &str) -> Result<bool>;

    /// Stores the outputs identified by `source` under `key`.
    ///
    /// Returns `true` on success. Errors are propagated to the caller so
    /// they can be surfaced as non-fatal warnings.
    fn save(&self, key: &str, source: &[PathBuf], metadata: CacheMetadata) -> Result<bool>;
}

/// Builds the concrete [`CacheProvider`] described by the config.
///
/// The returned `Arc<dyn CacheProvider>` is the single instance shared for
/// the lifetime of the run.
#[must_use]
pub fn build_cache_provider(kind: &CacheProviderKind) -> Arc<dyn CacheProvider> {
    match kind {
        CacheProviderKind::Local => Arc::new(LocalCacheProvider::default()),
    }
}
