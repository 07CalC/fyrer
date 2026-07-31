pub mod config;
pub mod graph;

use thiserror::Error;

use crate::error::config::ConfigError;
use crate::error::graph::GraphError;

#[derive(Debug, Error)]
pub enum FyrerError {
    #[error("config error: {0}")]
    Config(#[from] ConfigError),
    #[error("graph error: {0}")]
    Graph(#[from] GraphError),
}

pub type FyrerResult<T> = Result<T, FyrerError>;
