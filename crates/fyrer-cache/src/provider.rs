use std::path::PathBuf;

use anyhow::Result;

use crate::hash::{CacheKey, OutputDigest};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheMetadata {
    pub task: String,
    pub duration_ms: u128,
    pub exit_code: i32,
    pub cache_status: CacheStatus,
    pub cache_key: CacheKey,
    pub output_digest: OutputDigest,
    pub timestamp: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CacheStatus {
    Hit,
    Miss,
}

impl CacheMetadata {
    pub fn new(
        task: String,
        duration_ms: u128,
        exit_code: i32,
        cache_status: CacheStatus,
        cache_key: CacheKey,
        output_digest: OutputDigest,
        timestamp: u64,
    ) -> Self {
        Self {
            task,
            duration_ms,
            exit_code,
            cache_status,
            cache_key,
            output_digest,
            timestamp,
        }
    }
}

// Async trait (architecture calls for async). Use async-trait via native async fn in trait
// (requires Rust 1.75+ with async fn in trait which we have on edition 2024).
pub trait CacheProvider: Send + Sync {
    fn contains(&self, key: &str) -> bool;
    fn restore(&self, key: &str, cwd: &std::path::Path) -> Result<bool>;
    fn save(&self, key: &str, source: &[PathBuf], metadata: CacheMetadata) -> Result<bool>;
    fn get_metadata(&self, key: &str) -> Result<Option<CacheMetadata>>;
    fn need_hydration(&self, key: &str, output_digest: &str) -> Result<bool>;
}

// For backwards compat with old call sites that don't pass cwd, provide delegated impl
#[allow(dead_code)]
pub fn restore_legacy<P: CacheProvider>(provider: &P, key: &str) -> Result<bool> {
    provider.restore(key, &std::env::current_dir()?)
}
