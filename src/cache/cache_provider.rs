use std::path::PathBuf;

use anyhow::Result;

pub trait CacheProvider {
    fn restore(&self, key: &str) -> Result<bool>;
    fn save(&self, key: &str, source: &[PathBuf]) -> Result<bool>;
    fn contains(&self, key: &str) -> bool;
}
