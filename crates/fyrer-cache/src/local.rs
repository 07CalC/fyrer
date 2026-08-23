//! Local content-addressed cache: outputs archived as `tar.zst` under
//! `.fyrer/cache/<key>/`, with a `meta.json` holding the output digest.

use std::{fs::File, path::{Path, PathBuf}};

use anyhow::Context;

use crate::provider::{CacheMetadata, CacheProvider};

pub static DEFAULT_CACHE_DIR: &str = ".fyrer/cache";
static ARCHIVE_NAME: &str = "outputs.tar.zst";

pub struct LocalCacheProvider {
    cache_dir: String,
}

impl LocalCacheProvider {
    pub fn new(cache_dir: String) -> Self {
        Self { cache_dir }
    }

    fn key_path(&self, key: &str) -> PathBuf {
        Path::new(&self.cache_dir).join(key)
    }
}

impl CacheProvider for LocalCacheProvider {
    fn contains(&self, key: &str) -> bool {
        let path = self.key_path(key);
        path.exists() && path.is_dir()
    }

    /// Restore cached outputs into the task's working directory.
    ///
    /// Archives are unpacked preserving their stored relative paths; the
    /// legacy flat-file layout (pre-archive caches) is still accepted.
    fn restore(&self, key: &str, cwd: &Path) -> anyhow::Result<bool> {
        let cache_path = self.key_path(key);
        if !cache_path.exists() {
            return Ok(false);
        }

        let archive = cache_path.join(ARCHIVE_NAME);
        if archive.exists() {
            let file = File::open(&archive)?;
            let decoder = zstd::stream::Decoder::new(file)?;
            for entry in tar::Archive::new(decoder).entries()? {
                entry?.unpack_in(cwd)?;
            }
            return Ok(true);
        }

        // Legacy layout: flat files next to meta.json.
        for entry in std::fs::read_dir(&cache_path)? {
            let path = entry?.path();
            if !path.is_file()
                || path.file_name() == Some(std::ffi::OsStr::new("meta.json"))
            {
                continue;
            }
            let name = path
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("unnamed cache file: {path:?}"))?;
            let dest = cwd.join(name);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&path, &dest)?;
        }
        Ok(true)
    }

    fn save(&self, key: &str, source: &[PathBuf], metadata: CacheMetadata) -> anyhow::Result<bool> {
        let final_path = self.key_path(key);
        std::fs::create_dir_all(&final_path)?;

        // Build in a sibling temp dir so the final rename is atomic.
        let temp_dir = tempfile::Builder::new()
            .prefix(&format!(".{key}."))
            .tempdir_in(&self.cache_dir)
            .context("failed to create temporary directory for cache")?;

        std::fs::write(
            temp_dir.path().join("meta.json"),
            serde_json::to_string(&metadata)?,
        )
        .context("failed to write cache metadata")?;

        write_archive(&temp_dir.path().join(ARCHIVE_NAME), source)?;

        if final_path.exists() {
            std::fs::remove_dir_all(&final_path)?;
        }
        std::fs::rename(temp_dir.path(), &final_path).with_context(|| {
            format!("failed to finalize cache dir {:?} -> {:?}", temp_dir.path(), final_path)
        })?;
        let _ = temp_dir.keep(); // moved away; don't delete on drop

        Ok(true)
    }

    fn get_metadata(&self, key: &str) -> anyhow::Result<Option<CacheMetadata>> {
        let path = self.key_path(key).join("meta.json");
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(serde_json::from_reader(File::open(path)?).context(
            "corrupted cache metadata",
        )?))
    }

    fn need_hydration(&self, key: &str, output_digest: &str) -> anyhow::Result<bool> {
        match self.get_metadata(key)? {
            Some(metadata) => Ok(metadata.output_digest != output_digest),
            None => Ok(true),
        }
    }
}

/// Tar+zstd the given output paths. Entries are stored by file name so a
/// restore lands them relative to the task cwd.
fn write_archive(archive_path: &Path, source: &[PathBuf]) -> anyhow::Result<()> {
    let file =
        File::create(archive_path).with_context(|| format!("create {:?}", archive_path))?;
    let encoder =
        zstd::stream::Encoder::new(file, 1).context("zstd encoder init")?;
    let mut tar = tar::Builder::new(encoder);

    for src in source {
        if !src.exists() {
            continue;
        }
        let name = src.file_name().unwrap_or(src.as_os_str());
        if src.is_dir() {
            tar.append_dir_all(name, src)
                .with_context(|| format!("append dir {src:?}"))?;
        } else {
            tar.append_path_with_name(src, name)
                .with_context(|| format!("append file {src:?}"))?;
        }
    }

    tar.into_inner()
        .context("finish tar archive")?
        .finish()
        .context("finish zstd stream")?;
    Ok(())
}
