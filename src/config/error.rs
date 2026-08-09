use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO error:\n {0}")]
    Io(#[from] std::io::Error),
    #[error("deserialization error:\n {0}")]
    Deserialization(#[from] serde_yaml::Error),
    #[error("validation error:\n {0}")]
    Validation(#[from] ValidationError),
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("unsupported version: {0}")]
    UnsupportedVersion(u32),
    #[error("duplicate package name: {0}")]
    DuplicatePackageName(String),
    #[error("empty root for package: {project}")]
    EmptyProjectRoot { project: String },
    #[error("Project root does not exist for package: {project}")]
    ProjectRootDoesNotExist { project: String },
    #[error("Absolute path is not allowed for package root: {project}")]
    AbsoluteProjectRoot { project: String },
    #[error("env_file not found: {file} for task: {task} in package: {project}")]
    EnvFileNotFound {
        project: String,
        task: String,
        file: String,
    },

    #[error("duplicate task name: {task} in package: {project}")]
    DuplicateTaskName { project: String, task: String },
    #[error("empty command for task: {task} in package: {project}")]
    EmptyCommand { project: String, task: String },
    #[error("timeout cannot be less than or equal to zero for task: {task} in package: {project}")]
    InvalidTimeout { project: String, task: String },
    #[error(
        "absolute path detected in {actor} for task: {task} in package: {project}. Absolute paths are not allowed."
    )]
    AbsolutePath {
        project: String,
        task: String,
        actor: String,
    },
    #[error(
        "invalid glob pattern detected in {actor} for task: {task} in package: {project}: {pattern}"
    )]
    InvalidGlobPattern {
        project: String,
        task: String,
        actor: String,
        pattern: String,
    },
    #[error(
        "task cwd is not a subdirectory of the package root for task: {task} in package: {project}"
    )]
    InvalidCwd { project: String, task: String },
    #[error("persistent tasks cannot be cached for task: {task} in package: {project}")]
    CacheWithPersistentTask { project: String, task: String },
    #[error("watch tasks cannot be cached for task: {task} in package: {project}")]
    CacheWithWatchTask { project: String, task: String },
}
