use std::{fs::File, path::Path};

use anyhow::Context;

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
        let path = Path::new(&self.cache_dir).join(key);
        path.exists() && path.is_dir()
    }

    fn restore(&self, key: &str) -> anyhow::Result<bool> {
        let cache_path = Path::new(&self.cache_dir).join(key);
        if !cache_path.exists() {
            return Ok(false);
        }
        for entry in std::fs::read_dir(&cache_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.file_name() != Some(std::ffi::OsStr::new("meta.json")) {
                let dest = std::env::current_dir()?.join(path.file_name().ok_or_else(|| {
                    anyhow::anyhow!("Failed to get file name from cache path: {:?}", path)
                })?);
                std::fs::copy(&path, &dest)?;
            }
        }
        Ok(true)
    }

    fn save(
        &self,
        key: &str,
        source: &[std::path::PathBuf],
        metadata: crate::cache::CacheMetadata,
    ) -> anyhow::Result<bool> {
        let cache_path = Path::new(&self.cache_dir).join(key);
        std::fs::create_dir_all(&cache_path)?;
        let temp_dir = tempfile::Builder::new()
            .prefix(".{key}.")
            .tempdir_in(&self.cache_dir)
            .context("Failed to create temporary directory for cache")?;
        std::fs::write(
            temp_dir.path().join("meta.json"),
            serde_json::to_string(&metadata)?,
        )
        .context("Failed to write cache meta")?;

        let archive_path = temp_dir.path().join("outputs.tar.zst");
        let file = File::create(&archive_path)
            .with_context(|| format!("Failed to create archive file: {:?}", archive_path))?;
        let encoder = zstd::stream::Encoder::new(file, 1)
            .context("Failed to create zstd encoder for cache archive")?;
        let mut tar = tar::Builder::new(encoder);

        for src in source {
            if !src.exists() {
                continue;
            }
            if src.is_dir() {
                tar.append_dir_all(src, src).with_context(|| {
                    format!("Failed to append directory {:?} to cache archive", src)
                })?;
            } else {
                tar.append_path_with_name(src, src)
                    .with_context(|| format!("Failed to append file {:?} to cache archive", src))?;
            }
        }

        let encoder = tar
            .into_inner()
            .context("Failed to finish writing tar archive")?;
        encoder
            .finish()
            .context("Failed to finish zstd encoding for cache archive")?;
        std::fs::rename(temp_dir.path(), &cache_path).with_context(|| {
            format!(
                "Failed to move temporary cache directory {:?} to final location {:?}",
                temp_dir.path(),
                cache_path
            )
        })?;
        let _ = temp_dir.keep();

        Ok(true)
    }

    fn get_metadata(&self, key: &str) -> anyhow::Result<Option<crate::cache::CacheMetadata>> {
        let metadata_path = Path::new(&self.cache_dir).join(key).join("meta.json");
        if metadata_path.exists() {
            let metadata_file = std::fs::File::open(metadata_path)?;
            let metadata: crate::cache::CacheMetadata = serde_json::from_reader(metadata_file)?;
            Ok(Some(metadata))
        } else {
            Ok(None)
        }
    }

    fn need_hydration(&self, key: &str, output_digest: &str) -> anyhow::Result<bool> {
        let metadata = self.get_metadata(key)?;
        if let Some(metadata) = metadata {
            Ok(metadata.output_digest != output_digest)
        } else {
            Ok(true)
        }
    }
}
