use thiserror::Error;

#[derive(Debug, Error)]
pub enum EnvError {
    #[error("environment variable '{0}' is not set")]
    VarNotSet(String),
    #[error("failed to parse environment variable '{var}' with value '{value}': {source}")]
    VarParse {
        var: String,
        value: String,
        #[source]
        source: std::num::ParseIntError,
    },
    #[error("failed to read environment variable '{var}': {source}")]
    VarRead {
        var: String,
        #[source]
        source: std::env::VarError,
    },
    #[error("failed to read env file: {source}")]
    IoError {
        #[from]
        source: std::io::Error,
    },
}
