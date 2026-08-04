use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::{
    env::{EnvMap, get_task_env_var, merge},
    error::{ConfigError, FyrerError, FyrerResult},
    tasks::{Task, TaskId, TaskMap},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FyrerConfig {
    pub version: u32,
    /// Environment variables shared by every project.
    #[serde(default = "default_env_map")]
    pub env: EnvMap,
    pub projects: Vec<ProjectConfig>,
}

/// A single project in the monorepo.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    /// Unique project name, used as the first half of every task id.
    pub name: String,
    /// Relative root directory of the project.
    pub root: PathBuf,
    /// Environment variables shared by every task in the project.
    #[serde(default = "default_env_map")]
    pub env: EnvMap,
    /// Path of the project's `.env` file, relative to `root`.
    #[serde(default = "default_env_path")]
    pub env_path: String,
    /// The tasks defined in this project, keyed by task name.
    #[serde(default = "default_tasks")]
    pub tasks: HashMap<String, TaskConfig>,
}

/// A task definition within a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskConfig {
    /// The shell command to run.
    #[serde(default = "default_cmd")]
    pub cmd: String,
    /// Names of tasks that must run first, either `name` or `project:name`.
    #[serde(default = "default_vec_string")]
    pub depends_on: Vec<String>,
    /// Glob patterns of files watched for changes.
    #[serde(default = "default_vec_string")]
    pub inputs: Vec<String>,
    /// Glob patterns of files produced by the task.
    #[serde(default = "default_vec_string")]
    pub outputs: Vec<String>,
    /// Glob patterns of files excluded from watching.
    #[serde(default = "default_vec_string")]
    pub ignore: Vec<String>,
    /// Whether the task may be skipped when its outputs are already fresh.
    #[serde(default = "default_bool")]
    pub cache: bool,
    /// How and when the task should be restarted.
    #[serde(default = "default_restart")]
    pub restart: RestartConfig,
    /// Environment variables that override the root and project-level env.
    #[serde(default = "default_env_map")]
    pub env: EnvMap,
}

/// Restart policy for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartConfig {
    /// When the task should be restarted.
    pub strategy: RestartStrategy,
    /// Debounce delay in milliseconds before restarting.
    pub delay: Option<u64>,
}

impl Default for RestartConfig {
    fn default() -> Self {
        Self {
            strategy: RestartStrategy::Never,
            delay: None,
        }
    }
}

/// When a task is restarted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum RestartStrategy {
    /// Restart whenever a watched input file changes.
    FileChange,
    /// Restart after the process exits with a failure.
    OnFailure,
    /// Never restart the task.
    #[default]
    Never,
}

impl FyrerConfig {
    /// Loads and validates a configuration from a YAML file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read, is not valid YAML, or
    /// fails validation.
    pub fn new_from_path(path: impl AsRef<Path>) -> FyrerResult<Self> {
        let content = std::fs::read_to_string(path.as_ref()).map_err(|source| {
            FyrerError::Config(ConfigError::ReadFile {
                path: path.as_ref().display().to_string(),
                source,
            })
        })?;
        Self::new_from_str(&content)
    }

    /// Parses and validates a configuration from a YAML string.
    ///
    /// # Errors
    ///
    /// Returns an error if the content is not valid YAML or fails validation.
    pub fn new_from_str(content: &str) -> FyrerResult<Self> {
        let config: Self = serde_yaml::from_str(content)
            .map_err(|source| FyrerError::Config(ConfigError::ParseYaml(source)))?;
        config.validate()?;
        Ok(config)
    }

    /// Builds the resolved task map from the raw configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if a project's env file cannot be read.
    pub fn create_task_map(&self) -> FyrerResult<TaskMap> {
        let mut task_map = HashMap::new();
        for project in &self.projects {
            let env_path = project.root.join(&project.env_path);
            let project_env = merge(&self.env, &project.env);
            for (task_name, task_config) in &project.tasks {
                let task = Task {
                    project_name: project.name.clone(),
                    project_root: project.root.clone(),
                    env: get_task_env_var(&project_env, &task_config.env, &env_path)?,
                    task_name: task_name.clone(),
                    cmd: task_config.cmd.clone(),
                    depends_on: task_config.depends_on.clone(),
                    inputs: task_config.inputs.clone(),
                    outputs: task_config.outputs.clone(),
                    ignore: task_config.ignore.clone(),
                    cache: task_config.cache,
                    restart: task_config.restart.clone(),
                };
                task_map.insert(TaskId::new(&project.name, task_name), task);
            }
        }
        Ok(task_map)
    }
    fn validate(&self) -> FyrerResult<()> {
        self.validate_version()?;
        self.validate_projects()?;
        self.validate_tasks()?;
        Ok(())
    }

    fn validate_version(&self) -> FyrerResult<()> {
        if self.version != 1 {
            return Err(FyrerError::Config(ConfigError::UnsupportedVersion {
                version: self.version,
            }));
        }
        Ok(())
    }

    fn validate_projects(&self) -> FyrerResult<()> {
        let mut project_names = HashSet::new();
        for project in &self.projects {
            if !project_names.insert(&project.name) {
                return Err(FyrerError::Config(ConfigError::DuplicateProject {
                    name: project.name.clone(),
                }));
            }
            if project.root.as_os_str().is_empty() {
                return Err(FyrerError::Config(ConfigError::EmptyProjectRoot {
                    project: project.name.clone(),
                }));
            }
            if project.root.is_absolute() {
                return Err(FyrerError::Config(ConfigError::AbsoluteProjectRoot {
                    project: project.name.clone(),
                    path: project.root.display().to_string(),
                }));
            }
            if project.env_path.is_empty() {
                return Err(FyrerError::Config(ConfigError::EmptyEnvPath {
                    project: project.name.clone(),
                }));
            }
        }
        Ok(())
    }

    fn validate_tasks(&self) -> FyrerResult<()> {
        for project in &self.projects {
            for (task_name, task) in &project.tasks {
                if task.cmd.is_empty() {
                    return Err(FyrerError::Config(ConfigError::EmptyCommand {
                        project: project.name.clone(),
                        task: task_name.clone(),
                    }));
                }

                if task.cache && task.outputs.is_empty() {
                    return Err(FyrerError::Config(ConfigError::CacheWithoutOutputs {
                        project: project.name.clone(),
                        task: task_name.clone(),
                    }));
                }

                if task.cache && task.inputs.is_empty() {
                    return Err(FyrerError::Config(ConfigError::CacheWithoutInputs {
                        project: project.name.clone(),
                        task: task_name.clone(),
                    }));
                }

                if task.restart.strategy == RestartStrategy::FileChange && task.inputs.is_empty() {
                    return Err(FyrerError::Config(ConfigError::FileChangeWithoutInputs {
                        project: project.name.clone(),
                        task: task_name.clone(),
                    }));
                }
            }
        }
        Ok(())
    }
}

trait TaskResolver {
    fn resolve(&self, spec: Option<&str>) -> Result<Vec<TaskId>>;
}

impl TaskResolver for TaskMap {
    fn resolve(&self, spec: Option<&str>) -> Result<Vec<TaskId>> {
        match spec {
            None => {
                let mut all: Vec<TaskId> = self.keys().cloned().collect();
                all.sort_by_key(ToString::to_string);
                Ok(all)
            }
            Some(spec) if spec.contains(":") => {
                let id = TaskId::parse(spec).ok_or_else(|| {
                    anyhow!(
                        "Invalid task specifier '{}'. Expected format 'project:task'.",
                        spec
                    )
                })?;
                if !self.contains_key(&id) {
                    return Err(anyhow!(
                        "Task '{}' not found in configuration.",
                        id.to_string()
                    ));
                }
                Ok(vec![id])
            }
            Some(spec) => {
                let mut matching_ids: Vec<TaskId> = self
                    .keys()
                    .filter(|id| id.task_name() == spec)
                    .cloned()
                    .collect();
                if matching_ids.is_empty() {
                    return Err(anyhow!(
                        "No tasks found with name '{}' in configuration.",
                        spec
                    ));
                }
                matching_ids.sort_by_key(ToString::to_string);
                Ok(matching_ids)
            }
        }
    }
}

fn default_vec_string() -> Vec<String> {
    Vec::new()
}

fn default_env_map() -> EnvMap {
    HashMap::new()
}

fn default_env_path() -> String {
    ".env".to_string()
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
    RestartConfig::default()
}
