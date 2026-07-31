use thiserror::Error;

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
    #[error("failed to send log message: {0}")]
    LogSend(#[from] tokio::sync::mpsc::error::SendError<crate::logger::LogMessage>),
}
