use std::path::PathBuf;

use anyhow::Result;

use crate::cache::CacheMetadata;

pub trait CacheProvider {
    fn restore(&self, key: &str) -> Result<bool>;
    fn save(&self, key: &str, source: &[PathBuf], metadata: CacheMetadata) -> Result<bool>;
    fn contains(&self, key: &str) -> bool;
}
