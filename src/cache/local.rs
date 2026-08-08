use std::{fs::File, path::Path};

use anyhow::anyhow;

use crate::{
    cache::{CacheMetadata, cache_provider::CacheProvider},
    config::DEFAULT_FYRER_DIR,
};

#[derive(Default)]
pub struct LocalCacheProvider;

impl CacheProvider for LocalCacheProvider {
    fn restore(&self, key: &str) -> anyhow::Result<bool> {
        let fyrer_dir = Path::new(DEFAULT_FYRER_DIR);
        let cache_dir = fyrer_dir.join("cache").join(key);
        if !cache_dir.exists() {
            return Ok(false);
        }
        let Some(file) = File::open(cache_dir.join("outputs.tar.zst")).ok() else {
            return Ok(false);
        };

        let Some(decoder) = zstd::Decoder::new(file).ok() else {
            return Ok(false);
        };
        let mut archive = tar::Archive::new(decoder);
        if archive.unpack(".").is_err() {
            return Ok(false);
        }
        Ok(true)
    }
    fn save(
        &self,
        key: &str,
        source: &[std::path::PathBuf],
        metadata: CacheMetadata,
    ) -> anyhow::Result<bool> {
        let fyrer_dir = Path::new(DEFAULT_FYRER_DIR);
        std::fs::create_dir_all(fyrer_dir)?;

        let cache_root = fyrer_dir.join("cache");
        std::fs::create_dir_all(&cache_root)?;

        let cache_dir = cache_root.join(key);

        if cache_dir.exists() {
            return Err(anyhow!("Cache directory already exists: {:?}", cache_dir));
        }

        // create a temporary directory to write the cache files before renaming it to the actual cache directory. this ensures atomicity
        let temp_dir = tempfile::Builder::new()
            .prefix(&format!(".{key}."))
            .tempdir_in(&cache_root)?;

        std::fs::write(
            temp_dir.path().join("meta.json"),
            serde_json::to_string(&metadata)?,
        )?;

        let file = File::create(temp_dir.path().join("outputs.tar.zst"))?;
        let encoder = zstd::Encoder::new(file, 1)?;
        let mut tar = tar::Builder::new(encoder);

        for output in source {
            if output.exists() {
                tar.append_dir_all(output, output)?;
            }
        }

        let encoder = tar.into_inner()?;
        encoder.finish()?;

        // publish the cache by renaming the temp file to actual file
        std::fs::rename(temp_dir.path(), &cache_dir)?;

        // prevent tempfile from trying to remove the directory after rename.
        temp_dir.keep();

        Ok(true)
    }
    fn contains(&self, key: &str) -> bool {
        let fyrer_dir = Path::new(DEFAULT_FYRER_DIR);
        let cache_dir = fyrer_dir.join("cache").join(key).is_dir();
        cache_dir
    }
}
