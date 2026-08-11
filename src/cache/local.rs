use crate::cache::CacheProvider;

pub static DEFAULT_CACHE_DIR: &str = ".fyrer/cache";
pub struct LocalCacheProvider {
    cache_dir: String,
}

impl LocalCacheProvider {
    pub fn new(cache_dir: String) -> Self {
        Self { cache_dir }
    }
}

impl CacheProvider for LocalCacheProvider {
    fn contains(&self, key: &str) -> bool {
        false
    }

    fn restore(&self, key: &str) -> anyhow::Result<bool> {
        todo!()
    }

    fn save(
        &self,
        key: &str,
        source: &[std::path::PathBuf],
        metadata: crate::cache::CacheMetadata,
    ) -> anyhow::Result<bool> {
        todo!()
    }

    fn get_metadata(&self, key: &str) -> anyhow::Result<Option<crate::cache::CacheMetadata>> {
        todo!()
    }

    fn need_hydration(&self, output_digest: &str) -> anyhow::Result<bool> {
        todo!()
    }
}
