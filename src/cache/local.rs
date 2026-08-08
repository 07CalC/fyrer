use std::{fs::File, path::Path};

use anyhow::anyhow;

use crate::{
    cache::{CacheMetadata, cache_provider::CacheProvider},
    config::DEFAULT_FYRER_DIR,
};

#[derive(Default)]
pub struct LocalCacheProvider;

impl CacheProvider for LocalCacheProvider {
    fn restore(&self, _: &str) -> anyhow::Result<bool> {
        Ok(true)
    }
    fn save(
        &self,
        key: &str,
        source: &[std::path::PathBuf],
        metadata: CacheMetadata,
    ) -> anyhow::Result<bool> {
        let fyrer_dir = Path::new(DEFAULT_FYRER_DIR);
        if !fyrer_dir.exists() {
            std::fs::create_dir_all(fyrer_dir)?;
        }
        let cache_dir = fyrer_dir.join("cache").join(key);
        if cache_dir.exists() {
            return Err(anyhow!("Cache directory already exists: {:?}", cache_dir));
        }
        std::fs::create_dir_all(&cache_dir)?;
        std::fs::write(
            cache_dir.join("meta.json"),
            serde_json::to_string(&metadata)?,
        )?;

        let file = File::create(cache_dir.join("outputs.tar.zst"))?;
        let encoder = zstd::Encoder::new(file, 1)?;
        let mut tar = tar::Builder::new(encoder);
        for output in source {
            if output.exists() {
                tar.append_dir_all(output, output)?;
            }
        }
        let encoder = tar.into_inner()?;
        encoder.finish()?;
        Ok(true)
    }
    fn contains(&self, key: &str) -> bool {
        let fyrer_dir = Path::new(".fyrer");
        let cache_dir = fyrer_dir.join("cache").join(key).is_dir();
        cache_dir
    }
}
