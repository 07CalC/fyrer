use std::{collections::HashMap, path::PathBuf};

use serde::Deserialize;

use crate::{config::task::TaskConfig, env::EnvMap};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageConfig {
    pub name: String,
    pub root: PathBuf,
    #[serde(default)]
    pub env: EnvMap,
    #[serde(default = "default_env_path")]
    pub env_file: String,
    #[serde(default = "default_tasks")]
    pub tasks: HashMap<String, TaskConfig>,
}

fn default_env_path() -> String {
    ".env".to_string()
}

fn default_tasks() -> HashMap<String, TaskConfig> {
    HashMap::new()
}
