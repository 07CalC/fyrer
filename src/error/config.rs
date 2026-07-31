use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file at '{path}': {source}")]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse YAML config: {0}")]
    ParseYaml(#[from] serde_yaml::Error),
    #[error("unsupported config version '{version}', expected 1")]
    UnsupportedVersion { version: u32 },
    #[error("duplicate project name '{name}'")]
    DuplicateProject { name: String },
    #[error("project '{project}' has empty root path")]
    EmptyProjectRoot { project: String },
    #[error("project '{project}' has absolute root path '{path}'. Root must be relative")]
    AbsoluteProjectRoot { project: String, path: String },
    #[error("project '{project}' has empty env_path")]
    EmptyEnvPath { project: String },
    #[error("task '{task}' in project '{project}' has empty cmd")]
    EmptyCommand { project: String, task: String },
    #[error("task '{task}' in project '{project}' has cache enabled but no outputs defined")]
    CacheWithoutOutputs { project: String, task: String },
    #[error("task '{task}' in project '{project}' cannot be both cacheable and persistent")]
    CacheAndPersistent { project: String, task: String },
    #[error(
        "task '{task}' in project '{project}' uses the FileChange restart strategy but has no inputs defined"
    )]
    FileChangeWithoutInputs { project: String, task: String },
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    #[error("{0}")]
    Other(String),
}
