use std::fs::read_to_string;

use crate::error::FyrerResult;

pub fn dir_exists(path: &str) -> bool {
    std::path::Path::new(path).is_dir()
}

pub fn file_exists(path: &str) -> bool {
    std::path::Path::new(path).is_file()
}

pub fn read_file(path: &str) -> FyrerResult<String> {
    let content = read_to_string(path).map_err(|e| {
        crate::error::FyrerError::Config(crate::error::ConfigError::ReadFile {
            path: path.to_string(),
            source: e,
        })
    })?;
    Ok(content)
}
