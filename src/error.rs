use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvnError {
    #[error("Failed to read env file:\n {source}")]
    ReadFile {
        #[source]
        source: std::io::Error,
    },
}
