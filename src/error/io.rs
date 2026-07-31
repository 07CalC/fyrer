use thiserror::Error;

#[derive(Debug, Error)]
pub enum IoError {
    #[error("failed to read file at '{path}': {source}")]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write file at '{path}': {source}")]
    WriteFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to create directory at '{path}': {source}")]
    CreateDir {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("file or directory not found at '{0}'")]
    NotFound(String),
    #[error("permission denied while accessing '{path}': {source}")]
    PermissionDenied {
        path: String,
        #[source]
        source: std::io::Error,
    },
}
