use std::{fs, process};

use crate::config::FyrerConfig;

pub fn load_config(path: &str) -> FyrerConfig {
    let data = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => {
            eprintln!("Failed to read configuration file: {}", path);
            process::exit(1)
        }
    };
    match serde_yaml::from_str(&data) {
        Ok(config) => config,
        Err(e) => {
            eprintln!(
                "Failed to parse configuration file: {}\n incorrect YAML format",
                e
            );
            process::exit(1)
        }
    }
}
