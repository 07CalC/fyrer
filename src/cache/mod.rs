use serde::{Deserialize, Serialize};

pub mod archive;
pub mod cache_provider;
pub mod hash;
pub mod local;

pub use cache_provider::CacheProvider;

use crate::cache::hash::{CacheKey, OutputDigest};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct CacheMetadata {
    pub task: String,
    pub duration_ms: u64,
    pub exit_code: i32,
    pub cache_status: CacheStatus,
    pub cache_key: CacheKey,
    pub output_digest: OutputDigest,
    pub timestamp: u64,
}

impl CacheMetadata {
    pub fn new(
        task: String,
        duration_ms: u64,
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

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum CacheStatus {
    Hit,
    Miss,
}
