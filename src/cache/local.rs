use std::{fs::File, path::Path};

use anyhow::Context;

use crate::{
    cache::{CacheMetadata, cache_provider::CacheProvider},
    config::DEFAULT_FYRER_DIR,
};

/// Cache provider that stores artifacts on the local filesystem under
/// `.fyrer/cache/<key>/`.
#[derive(Default)]
pub struct LocalCacheProvider;

impl CacheProvider for LocalCacheProvider {
    fn contains(&self, key: &str) -> bool {
        Path::new(DEFAULT_FYRER_DIR)
            .join("cache")
            .join(key)
            .is_dir()
    }

    fn restore(&self, key: &str, output_hash: &str) -> anyhow::Result<bool> {
        let start = std::time::Instant::now();
        let cache_dir = Path::new(DEFAULT_FYRER_DIR).join("cache").join(key);
        if !cache_dir.exists() {
            return Ok(false);
        }

        let meta_path = cache_dir.join("meta.json");
        let meta_file = File::open(&meta_path).with_context(|| {
            format!(
                "failed to open cache metadata file '{}'",
                meta_path.display()
            )
        })?;
        let metadata: CacheMetadata = serde_json::from_reader(meta_file).with_context(|| {
            format!(
                "failed to parse cache metadata file '{}'",
                meta_path.display()
            )
        })?;
        if metadata.output_hash == output_hash {
            println!("Cache entry '{}' is up to date (output hash matches)", key);
            return Ok(true);
        }

        let archive_path = cache_dir.join("outputs.tar.zst");
        let file = match File::open(&archive_path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => {
                return Err(e).with_context(|| {
                    format!("failed to open cache archive '{}'", archive_path.display())
                });
            }
        };

        let decoder = zstd::Decoder::new(file).with_context(|| {
            format!(
                "failed to create zstd decoder for '{}'",
                archive_path.display()
            )
        })?;
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(".").with_context(|| {
            format!(
                "failed to unpack cache archive '{}'",
                archive_path.display()
            )
        })?;

        let elapsed = start.elapsed();
        println!("Restored cache entry '{}' in {:.2?}", key, elapsed);
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
            // Idempotent: a concurrent run or retry already wrote this entry.
            return Ok(true);
        }

        // Write to a temp directory first; rename atomically to prevent
        // partial entries being observed by concurrent readers.
        let temp_dir = tempfile::Builder::new()
            .prefix(&format!(".{key}."))
            .tempdir_in(&cache_root)
            .context("failed to create temporary cache directory")?;

        std::fs::write(
            temp_dir.path().join("meta.json"),
            serde_json::to_string(&metadata).context("failed to serialise cache metadata")?,
        )
        .context("failed to write cache metadata")?;

        let archive_path = temp_dir.path().join("outputs.tar.zst");
        let file = File::create(&archive_path)
            .with_context(|| format!("failed to create archive '{}'", archive_path.display()))?;
        let encoder = zstd::Encoder::new(file, 1).context("failed to create zstd encoder")?;
        let mut tar = tar::Builder::new(encoder);

        for output in source {
            if output.exists() {
                tar.append_dir_all(output, output)
                    .with_context(|| format!("failed to archive '{}'", output.display()))?;
            }
        }

        let encoder = tar.into_inner().context("failed to flush tar archive")?;
        encoder.finish().context("failed to finish zstd encoding")?;

        // Publish: rename temp dir to the final location.
        std::fs::rename(temp_dir.path(), &cache_dir)
            .with_context(|| format!("failed to publish cache to '{}'", cache_dir.display()))?;

        // Prevent tempfile from trying to remove the (now-renamed) directory.
        let _ = temp_dir.keep();

        Ok(true)
    }
}
