use std::{collections::HashMap, path::Path};

use crate::error::{EnvError, FyrerError, FyrerResult};

/// A map of environment variable names to values.
pub type EnvMap = HashMap<String, String>;

/// Computes the final environment for a task.
///
/// The project level env consists global env already, the Precedence (low to high) is as follows:
///
/// 1. Global environment variables (injected to every proejct every task)
/// 2. Project level env variables (from `env` field in project config)
/// 3. `.env` file variables (from `.env` file in project root)
/// 4. Task level env variables (from `env` field in task config)
///
/// # Errors
///
/// Returns an error if the env file exists but cannot be read.
pub fn get_task_env_var(
    project_env: &EnvMap,
    task_env: &EnvMap,
    env_file_path: &Path,
) -> FyrerResult<EnvMap> {
    let mut env = project_env.clone();
    if env_file_path.exists() {
        env.extend(read_env_file(env_file_path)?);
    }
    env.extend(task_env.clone());
    Ok(env)
}

/// Merges `overrides` on top of `base`.
#[must_use]
pub fn merge(base: &EnvMap, overrides: &EnvMap) -> EnvMap {
    let mut merged = base.clone();
    merged.extend(overrides.clone());
    merged
}

/// Reads a `.env` file from disk.
///
/// returns a map of environment variable names to values. Blank lines and lines starting
/// with `#` are ignored.
///
/// # Errors
///
/// Returns an error if the file cannot be read.
pub fn read_env_file(path: &Path) -> FyrerResult<EnvMap> {
    let content = std::fs::read_to_string(path)
        .map_err(|source| FyrerError::Env(EnvError::ReadFile { source }))?;
    Ok(parse_env(&content))
}

/// Parses `KEY=VALUE` lines from a `.env` string.
///
/// Blank lines and lines starting with `#` are ignored.
#[must_use]
pub fn parse_env(env_str: &str) -> EnvMap {
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

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, path::Path};

    use super::{EnvMap, get_task_env_var, merge, parse_env};

    #[test]
    fn parses_key_value_lines() {
        let env = parse_env("FOO=bar\nBAZ = qux\nEMPTY=\n");
        assert_eq!(env.get("FOO").map(String::as_str), Some("bar"));
        assert_eq!(env.get("BAZ").map(String::as_str), Some("qux"));
        assert_eq!(env.get("EMPTY").map(String::as_str), Some(""));
    }

    #[test]
    fn skips_comments_and_blank_lines() {
        let env = parse_env("# a comment\n\n  # indented comment\nFOO=bar\n");
        assert_eq!(env.len(), 1);
        assert_eq!(env.get("FOO").map(String::as_str), Some("bar"));
    }

    #[test]
    fn skips_lines_without_equals() {
        let env = parse_env("NOT_A_PAIR\nFOO=bar\n");
        assert_eq!(env.len(), 1);
    }

    #[test]
    fn ignores_empty_keys() {
        let env = parse_env("=value\nFOO=bar\n");
        assert_eq!(env.len(), 1);
    }

    #[test]
    fn merge_applies_overrides_without_mutating_inputs() {
        let base = EnvMap::from([("A".to_string(), "1".to_string())]);
        let overrides = EnvMap::from([("A".to_string(), "2".to_string())]);

        let merged = merge(&base, &overrides);
        assert_eq!(merged["A"], "2");
        assert_eq!(base["A"], "1");
    }

    #[test]
    fn get_task_env_resolves_without_env_file() {
        let project_env = EnvMap::from([("A".to_string(), "1".to_string())]);
        let task_env = EnvMap::from([("B".to_string(), "2".to_string())]);
        let env = get_task_env_var(&project_env, &task_env, Path::new("/does/not/exist")).unwrap();
        assert_eq!(env["A"], "1");
        assert_eq!(env["B"], "2");
    }

    #[test]
    fn get_task_env_prefers_task_over_project() {
        let project_env = HashMap::from([("A".to_string(), "1".to_string())]);
        let task_env = HashMap::from([("A".to_string(), "2".to_string())]);
        let env = get_task_env_var(&project_env, &task_env, Path::new("/does/not/exist")).unwrap();
        assert_eq!(env["A"], "2");
    }
}
