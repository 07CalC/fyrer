use thiserror::Error;

#[derive(Debug, Error)]
pub enum StateError {
    #[error("global state has already been initialized")]
    AlreadyInitialized,
    #[error("global state has not been initialized yet")]
    NotInitialized,
}
