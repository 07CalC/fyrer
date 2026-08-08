use thiserror::Error;

pub type FyrerResult<T> = Result<T, FyrerError>;

#[derive(Debug, Error)]
pub enum FyrerError {
    #[error("config error: {0}")]
    Config(#[from] ConfigError),

    #[error("graph error: {0}")]
    Graph(#[from] GraphError),

    #[error("task error: {0}")]
    Task(#[from] TaskError),

    #[error("env error: {0}")]
    Env(#[from] EnvError),

    #[error("state error: {0}")]
    State(#[from] StateError),

    #[error("watcher error: {0}")]
    Watch(#[from] WatcherError),

    #[error("cache error: {0}")]
    Cache(#[from] CacheError),
}

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

    #[error("task '{task}' in project '{project}' has cache enabled but no inputs defined")]
    CacheWithoutInputs { project: String, task: String },

    #[error(
        "task '{task}' in project '{project}' uses the FileChange restart strategy but has no inputs defined"
    )]
    FileChangeWithoutInputs { project: String, task: String },
}

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("cycle detected involving task '{0}'")]
    CycleDetected(String),

    #[error("task '{dependency}' referenced by '{dependent}' not found")]
    MissingDependency {
        dependent: String,
        dependency: String,
    },

    #[error("task '{0}' depends on itself")]
    SelfDependency(String),

    #[error(
        "invalid task id '{task}' in dependency '{dependency}', expected format 'project:task'"
    )]
    InvalidTaskId { dependency: String, task: String },

    #[error("task '{0}' not found in the task graph")]
    TaskNotFound(String),
}

#[derive(Debug, Error)]
pub enum TaskError {
    #[error("failed to spawn command for task '{task}': {source}")]
    Spawn {
        task: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to wait for task '{task}' to finish: {source}")]
    Wait {
        task: String,
        #[source]
        source: std::io::Error,
    },

    #[error("task '{task}' exited with status {code}")]
    Failed { task: String, code: i32 },

    #[error("failed to read output of task '{task}': {source}")]
    ReadOutput {
        task: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to capture stdout of task '{0}'")]
    MissingStdout(String),

    #[error("failed to capture stderr of task '{0}'")]
    MissingStderr(String),

    #[error("task '{0}' not found in the task map")]
    NotFound(String),

    #[error("task '{task}' panicked: {source}")]
    Join {
        task: String,
        #[source]
        source: tokio::task::JoinError,
    },

    #[error("task '{0}' was cancelled by shutdown")]
    Cancelled(String),
}

#[derive(Debug, Error)]
pub enum EnvError {
    #[error("failed to read env file: {source}")]
    ReadFile {
        #[from]
        source: std::io::Error,
    },
}

#[derive(Debug, Error)]
pub enum StateError {
    #[error("global state has already been initialized")]
    AlreadyInitialized,
}

#[derive(Debug, Error)]
pub enum WatcherError {
    #[error("failed to initialize file watcher: {0}")]
    Init(#[from] notify::Error),

    #[error("failed to resolve project root '{path}': {source}")]
    ResolveRoot {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("project root '{0}' is not a directory")]
    MissingRoot(String),
}

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("failed to expand input glob '{pattern}': {source}")]
    Glob {
        pattern: String,
        #[source]
        source: glob::PatternError,
    },

    #[error("failed to read input file '{path}': {source}")]
    ReadInput {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to write cache file '{path}': {source}")]
    Write {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_render_their_domain() {
        let err = FyrerError::Config(ConfigError::DuplicateProject {
            name: "web".to_string(),
        });
        assert_eq!(
            err.to_string(),
            "config error: duplicate project name 'web'"
        );

        let err = FyrerError::Graph(GraphError::TaskNotFound("api:build".to_string()));
        assert_eq!(
            err.to_string(),
            "graph error: task 'api:build' not found in the task graph"
        );

        let err = FyrerError::State(StateError::AlreadyInitialized);
        assert_eq!(
            err.to_string(),
            "state error: global state has already been initialized"
        );
    }

    #[test]
    fn question_mark_converts_domain_errors() {
        fn takes_result() -> FyrerResult<()> {
            Err(ConfigError::UnsupportedVersion { version: 2 })?;
            Ok(())
        }
        assert!(matches!(
            takes_result().unwrap_err(),
            FyrerError::Config(ConfigError::UnsupportedVersion { version: 2 })
        ));
    }
}
