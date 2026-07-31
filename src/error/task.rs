use thiserror::Error;

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
}
