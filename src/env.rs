use std::{collections::HashMap, path::Path};

use anyhow::Result;

pub type EnvMap = HashMap<String, String>;

/// we assumes that if env_file_path is given, then if exists, this validation is already done
/// in the config validation phase, so we don't need to check for existence here
///
///
/// priority: less to more:
/// 1. global env
/// 2. project_env_file
/// 3. project_env
/// 4. task_env_file
/// 5. task_env
pub fn merge_envs(
    global_env: &EnvMap,
    project_env: &EnvMap,
    task_env: &EnvMap,
    project_env_file: Option<&Path>,
    task_env_file: Option<&Path>,
) -> EnvMap {
    let project_file_env = project_env_file
        .and_then(|path| read_env_file(path).ok())
        .unwrap_or_default();
    let task_file_env = task_env_file
        .and_then(|path| read_env_file(path).ok())
        .unwrap_or_default();

    let mut merged_env = merge(global_env, &project_file_env);
    merged_env = merge(&merged_env, project_env);
    merged_env = merge(&merged_env, &task_file_env);
    merged_env = merge(&merged_env, task_env);
    merged_env
}

#[must_use]
fn merge(base: &EnvMap, overrides: &EnvMap) -> EnvMap {
    let mut merged = base.clone();
    merged.extend(overrides.clone());
    merged
}

fn read_env_file(path: &Path) -> Result<EnvMap> {
    let content = std::fs::read_to_string(path)
        .map_err(|source| crate::error::EvnError::ReadFile { source })?;
    Ok(parse_env(&content))
}

#[must_use]
fn parse_env(env_str: &str) -> EnvMap {
    let mut env_map = EnvMap::new();
    for line in env_str.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            if !key.is_empty() {
                env_map.insert(key.to_string(), value.trim().to_string());
            }
        }
    }
    env_map
}
