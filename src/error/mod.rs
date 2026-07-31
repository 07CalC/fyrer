pub mod config;
pub mod env;
pub mod graph;
pub mod io;
pub mod logger;
pub mod state;
pub mod task;
pub mod watch;

use thiserror::Error;

use crate::error::config::ConfigError;
use crate::error::graph::GraphError;
use crate::error::io::IoError;
use crate::error::logger::LoggerError;
use crate::error::state::StateError;
use crate::error::task::TaskError;
use crate::error::watch::WatcherError;

#[derive(Debug, Error)]
pub enum FyrerError {
    #[error("config error: {0}")]
    Config(#[from] ConfigError),
    #[error("graph error: {0}")]
    Graph(#[from] GraphError),
    #[error("task error: {0}")]
    Task(#[from] TaskError),
    #[error("io error: {0}")]
    Io(#[from] IoError),
    #[error("state error: {0}")]
    State(#[from] StateError),
    #[error("logger error: {0}")]
    Logger(#[from] LoggerError),
    #[error("watcher error: {0}")]
    Watch(#[from] WatcherError),
    #[error("env error: {0}")]
    Env(#[from] env::EnvError),
}

pub type FyrerResult<T> = Result<T, FyrerError>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::config::ConfigError;
    use crate::error::graph::GraphError;
    use crate::error::io::IoError;
    use crate::error::state::StateError;

    #[test]
    fn test_fyrer_error_renders_domain() {
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

        let err = FyrerError::Io(IoError::NotFound("fyrer.yml".to_string()));
        assert_eq!(
            err.to_string(),
            "io error: file or directory not found at 'fyrer.yml'"
        );

        let err = FyrerError::State(StateError::AlreadyInitialized);
        assert_eq!(
            err.to_string(),
            "state error: global state has already been initialized"
        );
    }

    #[test]
    fn test_question_mark_conversion() {
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
