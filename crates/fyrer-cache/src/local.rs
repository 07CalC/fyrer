//! Local content-addressed cache: outputs archived as `tar.zst` under
//! `.fyrer/cache/<key>/`, with a `meta.json` holding the output digest.
//!
//! Archive entries are stored **relative to the workspace root** (the
//! directory containing the config file), and restore unpacks them back into
//! that same root. This keeps the full on-disk layout intact —
//! `packages/app/dist/…` stays `packages/app/dist/…` — no matter which
//! package or task produced the outputs.

use std::{
    fs::File,
    path::{Path, PathBuf},
};

use anyhow::Context;

use crate::provider::{CacheMetadata, CacheProvider};

pub static DEFAULT_CACHE_DIR: &str = ".fyrer/cache";
static ARCHIVE_NAME: &str = "outputs.tar.zst";
static META_NAME: &str = "meta.json";

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

    /// Restore cached outputs into the workspace root.
    ///
    /// Archives unpack preserving their workspace-relative paths; the legacy
    /// flat-file layout (pre-archive caches) is still accepted.
    fn restore(&self, key: &str, workspace_root: &Path) -> anyhow::Result<bool> {
        let cache_path = self.key_path(key);
        if !cache_path.exists() {
            return Ok(false);
        }

        let archive = cache_path.join(ARCHIVE_NAME);
        if archive.exists() {
            let file = File::open(&archive)?;
            let decoder = zstd::stream::Decoder::new(file)?;
            // `unpack_in` refuses entries escaping the root (path traversal).
            for entry in tar::Archive::new(decoder).entries()? {
                entry?.unpack_in(workspace_root)?;
            }
            return Ok(true);
        }

        // Legacy layout: flat files next to meta.json.
        for entry in std::fs::read_dir(&cache_path)? {
            let path = entry?.path();
            if !path.is_file() || path.file_name() == Some(std::ffi::OsStr::new(META_NAME)) {
                continue;
            }
            let name = path
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("unnamed cache file: {path:?}"))?;
            let dest = workspace_root.join(name);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&path, &dest)?;
        }
        Ok(true)
    }

    fn save(
        &self,
        key: &str,
        source: &[PathBuf],
        workspace_root: &Path,
        metadata: CacheMetadata,
    ) -> anyhow::Result<bool> {
        let final_path = self.key_path(key);
        std::fs::create_dir_all(&final_path)?;

        // Build in a sibling temp dir so the final rename is atomic.
        let temp_dir = tempfile::Builder::new()
            .prefix(&format!(".{key}."))
            .tempdir_in(&self.cache_dir)
            .context("failed to create temporary directory for cache")?;

        std::fs::write(
            temp_dir.path().join(META_NAME),
            serde_json::to_string(&metadata)?,
        )
        .context("failed to write cache metadata")?;

        write_archive(&temp_dir.path().join(ARCHIVE_NAME), source, workspace_root)?;

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
        let path = self.key_path(key).join(META_NAME);
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(
            serde_json::from_reader(File::open(path)?).context("corrupted cache metadata")?,
        ))
    }

    fn need_hydration(&self, key: &str, output_digest: &str) -> anyhow::Result<bool> {
        match self.get_metadata(key)? {
            Some(metadata) => Ok(metadata.output_digest != output_digest),
            None => Ok(true),
        }
    }
}

/// Tar+zstd the given absolute output paths, storing each as a path relative
/// to `workspace_root`. Sources outside the root are skipped with a warning —
/// config validation normally forbids them anyway.
fn write_archive(archive_path: &Path, source: &[PathBuf], workspace_root: &Path) -> anyhow::Result<()> {
    let file =
        File::create(archive_path).with_context(|| format!("create {:?}", archive_path))?;
    let encoder =
        zstd::stream::Encoder::new(file, 1).context("zstd encoder init")?;
    let mut tar = tar::Builder::new(encoder);

    // Ancestor directory headers already written (exact-match dedupe).
    let mut ancestor_headers: std::collections::HashSet<PathBuf> =
        std::collections::HashSet::new();
    // Content directories archived recursively; sources under one are
    // already covered and can be skipped.
    let mut content_dirs: Vec<PathBuf> = Vec::new();

    for src in source {
        if !src.exists() {
            continue;
        }
        let Ok(rel) = src.strip_prefix(workspace_root) else {
            eprintln!(
                "[cache] skipping output outside the workspace root: {}",
                src.display()
            );
            continue;
        };

        // Skip anything already covered by an archived directory.
        if content_dirs.iter().any(|dir| rel.starts_with(dir)) {
            continue;
        }

        emit_parent_dirs(&mut tar, rel, workspace_root, &mut ancestor_headers)?;

        if src.is_dir() {
            tar.append_dir(rel, src)
                .with_context(|| format!("append dir {src:?}"))?;
            content_dirs.push(rel.to_path_buf());
            append_dir_all_relative(&mut tar, src, rel)?;
        } else {
            tar.append_path_with_name(src, rel)
                .with_context(|| format!("append file {src:?}"))?;
        }
    }

    tar.into_inner()
        .context("finish tar archive")?
        .finish()
        .context("finish zstd stream")?;
    Ok(())
}

/// Emit header entries for every ancestor directory of `rel` (shallowest
/// first) so extraction can recreate deep trees on a clean checkout.
fn emit_parent_dirs(
    tar: &mut tar::Builder<zstd::stream::Encoder<File>>,
    rel: &Path,
    workspace_root: &Path,
    emitted: &mut std::collections::HashSet<PathBuf>,
) -> anyhow::Result<()> {
    let mut ancestors = Vec::new();
    let mut cur = rel.parent();
    while let Some(dir) = cur {
        if dir.as_os_str().is_empty() {
            break;
        }
        ancestors.push(dir.to_path_buf());
        cur = dir.parent();
    }
    ancestors.reverse();
    for dir in ancestors {
        // Header-only entries: exact dedupe is enough — nesting is fine
        // because extraction creates each level explicitly.
        if emitted.insert(dir.clone()) {
            tar.append_dir(&dir, workspace_root.join(&dir))
                .with_context(|| format!("append ancestor dir {dir:?}"))?;
        }
    }
    Ok(())
}

/// Recursively add a directory's contents under its workspace-relative name.
fn append_dir_all_relative(
    tar: &mut tar::Builder<zstd::stream::Encoder<File>>,
    abs: &Path,
    rel: &Path,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(abs).with_context(|| format!("read dir {abs:?}"))? {
        let entry_path = entry?.path();
        let entry_rel = rel.join(entry_path.file_name().unwrap_or_default());
        if entry_path.is_dir() {
            tar.append_dir(&entry_rel, &entry_path)
                .with_context(|| format!("append dir {entry_path:?}"))?;
            append_dir_all_relative(tar, &entry_path, &entry_rel)?;
        } else {
            tar.append_path_with_name(&entry_path, &entry_rel)
                .with_context(|| format!("append file {entry_path:?}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{CacheMetadata, CacheStatus};

    fn metadata() -> CacheMetadata {
        CacheMetadata::new(
            "test".into(),
            0,
            0,
            CacheStatus::Miss,
            "k".into(),
            "digest".into(),
            0,
        )
    }

    #[test]
    fn round_trips_workspace_relative_layout() {
        let ws = tempfile::tempdir().unwrap();
        let root = ws.path();

        // Simulate two packages producing outputs in different subdirs.
        let app_dist = root.join("packages/app/dist");
        let ui_dist = root.join("packages/ui/dist/assets");
        std::fs::create_dir_all(&app_dist).unwrap();
        std::fs::create_dir_all(&ui_dist).unwrap();
        std::fs::write(app_dist.join("index.js"), "app").unwrap();
        std::fs::write(ui_dist.join("logo.svg"), "<svg/>").unwrap();
        std::fs::write(root.join("top.txt"), "top").unwrap();

        let cache_dir = tempfile::tempdir().unwrap();
        let provider = LocalCacheProvider::new(cache_dir.path().to_string_lossy().to_string());

        let sources = vec![
            root.join("packages/app/dist/index.js"),
            root.join("packages/ui/dist"),
            root.join("top.txt"),
        ];
        provider.save("k1", &sources, root, metadata()).unwrap();
        assert!(provider.contains("k1"));

        // Nuke the outputs, then restore.
        std::fs::remove_dir_all(root.join("packages")).unwrap();
        std::fs::remove_file(root.join("top.txt")).unwrap();
        assert!(provider.restore("k1", root).unwrap());

        assert_eq!(
            std::fs::read_to_string(root.join("packages/app/dist/index.js")).unwrap(),
            "app"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("packages/ui/dist/assets/logo.svg")).unwrap(),
            "<svg/>"
        );
        assert_eq!(std::fs::read_to_string(root.join("top.txt")).unwrap(), "top");

        // Digest check drives hydration decisions.
        assert!(!provider.need_hydration("k1", "digest").unwrap());
        assert!(provider.need_hydration("k1", "other").unwrap());
    }

    #[test]
    fn missing_key_is_a_clean_miss() {
        let cache_dir = tempfile::tempdir().unwrap();
        let ws = tempfile::tempdir().unwrap();
        let provider = LocalCacheProvider::new(cache_dir.path().to_string_lossy().to_string());
        assert!(!provider.contains("nope"));
        assert!(!provider.restore("nope", ws.path()).unwrap());
        assert!(provider.get_metadata("nope").unwrap().is_none());
    }
}

#[cfg(test)]
mod archive_tests {
    use super::*;

    #[test]
    fn writes_ancestors_files_and_dedupes_nested_sources() {
        let ws = tempfile::tempdir().unwrap();
        let root = ws.path();
        let app = root.join("packages/app/dist");
        std::fs::create_dir_all(app.join("assets")).unwrap();
        std::fs::write(app.join("index.js"), "js").unwrap();
        std::fs::write(app.join("assets/logo.svg"), "<svg/>").unwrap();

        let archive_out = tempfile::tempdir().unwrap();
        let archive = archive_out.path().join("a.tar.zst");

        // Deliberately overlapping sources: dir + file inside it.
        let sources = vec![app.join("assets"), app.join("index.js"), app.join("assets/logo.svg")];
        write_archive(&archive, &sources, root).unwrap();

        let names: Vec<String> = list_archive(&archive);
        println!("entries: {names:?}");

        assert!(names.contains(&"packages".to_string()), "{names:?}");
        assert!(names.contains(&"packages/app".to_string()), "{names:?}");
        assert!(names.contains(&"packages/app/dist".to_string()), "{names:?}");
        assert!(names.contains(&"packages/app/dist/assets".to_string()), "{names:?}");
        assert_eq!(
            names.iter().filter(|n| *n == "packages/app/dist/index.js").count(),
            1,
            "{names:?}"
        );
        assert_eq!(
            names.iter().filter(|n| *n == "packages/app/dist/assets/logo.svg").count(),
            1,
            "logo duplicated by nested source: {names:?}"
        );
    }

    fn list_archive(archive: &Path) -> Vec<String> {
        let f = File::open(archive).unwrap();
        let d = zstd::stream::Decoder::new(f).unwrap();
        tar::Archive::new(d)
            .entries()
            .unwrap()
            .map(|e| {
                e.unwrap()
                    .path()
                    .unwrap()
                    .to_string_lossy()
                    .trim_end_matches('/')
                    .to_string()
            })
            .collect()
    }
}

#[cfg(test)]
mod bisect {
    use super::*;

    #[test]
    fn ancestors_only() {
        let ws = tempfile::tempdir().unwrap();
        let root = ws.path();
        let deep = root.join("packages/app/dist/assets");
        std::fs::create_dir_all(&deep).unwrap();

        let out = tempfile::tempdir().unwrap();
        let archive = out.path().join("a.tar.zst");
        let file = File::create(&archive).unwrap();
        let enc = zstd::stream::Encoder::new(file, 1).unwrap();
        let mut tar = tar::Builder::new(enc);
        let mut emitted = std::collections::HashSet::new();
        emit_parent_dirs(&mut tar, Path::new("packages/app/dist/assets"), root, &mut emitted).unwrap();
        tar.into_inner().unwrap().finish().unwrap();

        let f = File::open(&archive).unwrap();
        let d = zstd::stream::Decoder::new(f).unwrap();
        let names: Vec<String> = tar::Archive::new(d)
            .entries().unwrap()
            .map(|e| e.unwrap().path().unwrap().to_string_lossy().to_string())
            .collect();
        println!("ancestor entries: {names:?}");
        assert_eq!(names.len(), 3);
    }
}
