use thiserror::Error;

#[derive(Debug, Error)]

pub enum ConfigError {
    #[error("failed to read file at '{path}': {source}")]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse yaml config: {0}")]
    ParseYaml(#[from] serde_yaml::Error),
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Error)]
pub enum FyrerError {
    #[error("config error: {0}")]
    Config(#[from] ConfigError),
}

pub type FyrerResult<T> = Result<T, FyrerError>;
