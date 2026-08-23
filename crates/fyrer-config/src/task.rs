use std::{path::PathBuf, time::Duration};

use serde::Deserialize;

pub use crate::env::EnvMap;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskConfig {
    pub cmd: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub outputs: Vec<String>,
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default)]
    pub cache: bool,
    #[serde(default)]
    pub env: EnvMap,
    #[serde(default)]
    pub env_file: Option<PathBuf>,
    #[serde(default)]
    pub persistent: bool,
    #[serde(default)]
    pub watch: bool,
    #[serde(default, with = "humantime_serde")]
    pub timeout: Option<Duration>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
}
