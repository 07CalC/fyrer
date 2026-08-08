use std::path::Path;

use crate::cache::cache_provider::CacheProvider;

pub struct LocalCacheProvider;

impl CacheProvider for LocalCacheProvider {
    fn restore(&self, _: &str) -> anyhow::Result<bool> {
        Ok(true)
    }
    fn save(&self, _: &str, _: &[std::path::PathBuf]) -> anyhow::Result<bool> {
        Ok(true)
    }
    fn contains(&self, key: &str) -> bool {
        let fyrer_dir = Path::new(".fyrer");
        let cache_dir = fyrer_dir.join("cache").join(key).is_dir();
        cache_dir
    }
}
