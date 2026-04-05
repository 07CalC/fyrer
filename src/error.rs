#[derive(Debug, thiserror::Error)]
pub enum FyrerError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Invalid config: {0}")]
    InvalidConfig(String),

    #[error("IO error")]
    Io(#[from] std::io::Error),
}
