use serde::{Deserialize, Serialize};

pub mod archive;
pub mod cache;
pub mod cache_provider;
pub mod local;

pub use cache_provider::{CacheProvider, build_cache_provider};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct CacheMetadata {
    pub task: String,
    pub hash: String,
    pub cmd: String,
    pub dependencies: Vec<String>,
    pub duration_ms: u64,
    pub exit_code: i32,
    pub outputs: Vec<String>,
    pub cache: CacheStatus,
    pub cache_key: Option<String>,
    pub timestamp: u64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum CacheStatus {
    Hit,
    Miss,
}
