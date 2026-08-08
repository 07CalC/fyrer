use std::{path::PathBuf, sync::Arc};

use anyhow::Result;

use crate::{
    cache::{CacheMetadata, local::LocalCacheProvider},
    config::CacheProviderKind,
};

pub trait CacheProvider: Send + Sync {
    fn contains(&self, key: &str) -> bool;

    fn restore(&self, key: &str, output_hash: &str) -> Result<bool>;

    fn save(&self, key: &str, source: &[PathBuf], metadata: CacheMetadata) -> Result<bool>;
}

#[must_use]
pub fn build_cache_provider(kind: &CacheProviderKind) -> Arc<dyn CacheProvider> {
    match kind {
        CacheProviderKind::Local => Arc::new(LocalCacheProvider::default()),
    }
}
