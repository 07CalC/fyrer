use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

use crate::error::FyrerResult;

pub type EnvMap = HashMap<String, String>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FyrerConfig {
    pub version: u32,
    #[serde(default = "default_env_map")]
    pub env: EnvMap,
    pub projects: Vec<ProjectConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    pub name: String,
    pub root: PathBuf,
    pub env: EnvMap,
    pub env_path: String,
    #[serde(default = "default_tasks")]
    pub tasks: HashMap<String, TaskConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskConfig {
    #[serde(default = "default_cmd")]
    pub cmd: String,
    #[serde(default = "default_vec_string")]
    pub depends_on: Vec<String>,
    #[serde(default = "default_vec_string")]
    pub inputs: Vec<String>,
    #[serde(default = "default_vec_string")]
    pub outputs: Vec<String>,
    #[serde(default = "default_bool")]
    pub watch: bool,
    #[serde(default = "default_vec_string")]
    pub ignore: Vec<String>,
    #[serde(default = "default_bool")]
    pub cache: bool,
    #[serde(default = "default_restart")]
    pub restart: RestartConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartConfig {
    pub strategy: RestartStrategy,
    pub delay: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RestartStrategy {
    FileChange,
    OnFailure,
    Never,
}

impl FyrerConfig {
    pub fn new_from_path(path: &str) -> FyrerResult<FyrerConfig> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            crate::error::FyrerError::Config(crate::error::ConfigError::ReadFile {
                path: path.to_string(),
                source: e,
            })
        })?;

        let config: FyrerConfig = serde_yaml::from_str(content.as_str()).map_err(|e| {
            crate::error::FyrerError::Config(crate::error::ConfigError::ParseYaml(e))
        })?;
        config.validate().map_err(|e| {
            crate::error::FyrerError::Config(crate::error::ConfigError::InvalidConfig(
                e.to_string(),
            ))
        })?;
        Ok(config)
    }

    pub fn new_from_str(content: &str) -> FyrerResult<FyrerConfig> {
        let config: FyrerConfig = serde_yaml::from_str(content).map_err(|e| {
            crate::error::FyrerError::Config(crate::error::ConfigError::ParseYaml(e))
        })?;
        config.validate().map_err(|e| {
            crate::error::FyrerError::Config(crate::error::ConfigError::InvalidConfig(
                e.to_string(),
            ))
        })?;
        Ok(config)
    }

    fn validate(&self) -> FyrerResult<()> {
        Ok(())
    }

    fn validate_projects(&self) -> FyrerResult<()> {
        Ok(())
    }
}
fn default_vec_string() -> Vec<String> {
    Vec::new()
}

fn default_env_map() -> EnvMap {
    HashMap::new()
}

fn default_tasks() -> HashMap<String, TaskConfig> {
    HashMap::new()
}

fn default_bool() -> bool {
    false
}

fn default_cmd() -> String {
    "echo from fyrer".to_string()
}

fn default_restart() -> RestartConfig {
    RestartConfig {
        strategy: RestartStrategy::Never,
        delay: None,
    }
}
