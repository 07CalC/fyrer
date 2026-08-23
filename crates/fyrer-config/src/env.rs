//! `.env` file parsing and layered environment merging.

use std::collections::HashMap;

/// Environment variables shared by packages and tasks.
pub type EnvMap = HashMap<String, String>;

/// Merge environment layers, lowest to highest precedence:
/// 1. global env
/// 2. project env_file
/// 3. project env
/// 4. task env_file
/// 5. task env
///
/// Env-file existence is validated during config validation, so missing files
/// are silently skipped here.
#[must_use]
pub fn merge_envs(
    global_env: &EnvMap,
    project_env: &EnvMap,
    task_env: &EnvMap,
    project_env_file: Option<&std::path::Path>,
    task_env_file: Option<&std::path::Path>,
) -> EnvMap {
    let mut merged = global_env.clone();
    if let Some(path) = project_env_file {
        merged.extend(read_env_file(path).unwrap_or_default());
    }
    merged.extend(project_env.clone());
    if let Some(path) = task_env_file {
        merged.extend(read_env_file(path).unwrap_or_default());
    }
    merged.extend(task_env.clone());
    merged
}

fn read_env_file(path: &std::path::Path) -> Result<EnvMap, std::io::Error> {
    Ok(parse_env(&std::fs::read_to_string(path)?))
}

/// Plain `KEY=VALUE` lines; blanks and `#` comments ignored.
#[must_use]
pub fn parse_env(content: &str) -> EnvMap {
    let mut map = EnvMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            if !key.is_empty() {
                map.insert(key.to_string(), value.trim().to_string());
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_env_files() {
        let env = parse_env("A=1\n# comment\n\nB = spaced \nBAD\nC=");
        assert_eq!(env.get("A"), Some(&"1".to_string()));
        assert_eq!(env.get("B"), Some(&"spaced".to_string()));
        assert!(!env.contains_key("BAD"));
        assert!(env.contains_key("C"));
    }

    #[test]
    fn later_layers_win() {
        let mut task = EnvMap::new();
        task.insert("X".to_string(), "task".to_string());
        let mut global = EnvMap::new();
        global.insert("X".to_string(), "global".to_string());
        let merged = merge_envs(&global, &EnvMap::new(), &task, None, None);
        assert_eq!(merged.get("X"), Some(&"task".to_string()));
    }
}
