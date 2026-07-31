use thiserror::Error;

use crate::logger::LogMessage;

#[derive(Debug, Error)]
pub enum LoggerError {
    #[error("failed to send log message for task '{task}': {source}")]
    Send {
        task: String,
        #[source]
        source: tokio::sync::mpsc::error::SendError<LogMessage>,
    },
}
