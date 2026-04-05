use crate::{config::types::FyrerConfig, error::FyrerError};
use std::fs;

pub fn load_config(path: &str) -> Result<FyrerConfig, FyrerError> {
    if fs::exists(path).unwrap_or(false) == false {
        return Err(FyrerError::FileNotFound(path.to_string()));
    }
    let data = fs::read_to_string(path)?;
    parse_config(&data)
}

fn parse_config(content: &str) -> Result<FyrerConfig, FyrerError> {
    match serde_yaml::from_str(content) {
        Ok(config) => Ok(config),
        Err(e) => return Err(FyrerError::InvalidConfig(e.to_string())),
    }
}
