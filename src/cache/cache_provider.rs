use std::path::PathBuf;

use anyhow::Result;

use crate::cache::CacheMetadata;

pub trait CacheProvider: Send + Sync {
    fn contains(&self, key: &str) -> bool;

    fn restore(&self, key: &str) -> Result<bool>;

    fn save(&self, key: &str, source: &[PathBuf], metadata: CacheMetadata) -> Result<bool>;

    fn get_metadata(&self, key: &str) -> Result<Option<CacheMetadata>>;

    fn need_hydration(&self, key: &str, output_digest: &str) -> Result<bool>;
}
