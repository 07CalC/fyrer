use std::{collections::HashMap, path::PathBuf};

use crate::error::{FyrerResult, env::EnvError, task::TaskError};

pub type EnvMap = HashMap<String, String>;

pub fn get_task_env_var(
    project: &EnvMap,
    task: &EnvMap,
    env_file_path: &PathBuf,
) -> FyrerResult<EnvMap> {
    let mut env = project.clone();
    if env_file_path.exists() {
        let file_env = read_env_file(env_file_path)?;
        env.extend(file_env);
    }
    env.extend(task.clone());
    Ok(env)
}
pub fn merge(base: &EnvMap, overrides: &EnvMap) -> EnvMap {
    let mut merged = base.clone();
    merged.extend(overrides.clone());
    merged
}
pub fn read_env_file(path: &PathBuf) -> FyrerResult<EnvMap> {
    let content = std::fs::read_to_string(path).map_err(|e| EnvError::IoError { source: e })?;
    Ok(parse_env(&content))
}

pub fn parse_env(env_str: &str) -> EnvMap {
    let mut env_map = EnvMap::new();
    if env_str.trim().is_empty() {
        return env_map;
    }
    for line in env_str.lines() {
        if line.starts_with("#") || line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once("=") {
            let key = k.trim();
            let val = v.trim();
            if !key.is_empty() {
                env_map.insert(key.to_string(), val.to_string());
            }
        }
    }
    env_map
}
