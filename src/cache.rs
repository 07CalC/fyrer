use std::{
    collections::BTreeMap,
    hash::{DefaultHasher, Hash, Hasher},
    path::{Path, PathBuf},
};

use glob::MatchOptions;

use crate::{
    error::{CacheError, FyrerError, FyrerResult},
    tasks::Task,
};

/// Directory, relative to the invocation directory, holding cache state.
const CACHE_DIR: &str = ".fyrer/cache";

const MATCH_OPTIONS: MatchOptions = MatchOptions {
    case_sensitive: true,
    require_literal_separator: true,
    require_literal_leading_dot: true,
};

/// Whether a task's outputs are already up to date with its inputs.
///
/// A task is fresh when the stored cache key matches the current key derived
/// from its command, environment, and input files, and every output glob
/// still matches at least one file. Any failure to read cache state or hash
/// inputs is treated as a miss, so caching never prevents a run.
#[must_use]
pub fn is_fresh(task: &Task) -> bool {
    is_fresh_at(Path::new(CACHE_DIR), task)
}

/// Records the current cache key for a task after a successful run.
///
/// # Errors
///
/// Returns an error if the task's inputs cannot be read or the key cannot be
/// written.
pub fn record(task: &Task) -> FyrerResult<()> {
    record_at(Path::new(CACHE_DIR), task)
}

fn is_fresh_at(cache_root: &Path, task: &Task) -> bool {
    let Ok(key) = compute_key(task) else {
        return false;
    };
    let Ok(stored) = std::fs::read_to_string(key_path(cache_root, task)) else {
        return false;
    };
    stored.trim() == key && outputs_exist(task)
}

fn record_at(cache_root: &Path, task: &Task) -> FyrerResult<()> {
    let key = compute_key(task)?;
    let path = key_path(cache_root, task);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| {
            FyrerError::Cache(CacheError::Write {
                path: path.display().to_string(),
                source,
            })
        })?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, &key).map_err(|source| {
        FyrerError::Cache(CacheError::Write {
            path: tmp.display().to_string(),
            source,
        })
    })?;
    std::fs::rename(&tmp, &path).map_err(|source| {
        FyrerError::Cache(CacheError::Write {
            path: path.display().to_string(),
            source,
        })
    })
}

fn key_path(cache_root: &Path, task: &Task) -> PathBuf {
    cache_root
        .join(&task.project_name)
        .join(format!("{}.key", task.task_name))
}

fn compute_key(task: &Task) -> FyrerResult<String> {
    let mut hasher = DefaultHasher::new();
    task.cmd.hash(&mut hasher);
    let env: BTreeMap<&String, &String> = task.env.iter().collect();
    for (key, value) in &env {
        key.hash(&mut hasher);
        value.hash(&mut hasher);
    }
    hash_strings(&mut hasher, &task.inputs);
    hash_strings(&mut hasher, &task.outputs);
    hash_strings(&mut hasher, &task.ignore);
    for path in matched_input_files(task)? {
        path.as_os_str().hash(&mut hasher);
        let bytes = std::fs::read(&path).map_err(|source| {
            FyrerError::Cache(CacheError::ReadInput {
                path: path.display().to_string(),
                source,
            })
        })?;
        bytes.hash(&mut hasher);
    }
    Ok(format!("{:016x}", hasher.finish()))
}

fn hash_strings(hasher: &mut DefaultHasher, values: &[String]) {
    let mut sorted: Vec<&String> = values.iter().collect();
    sorted.sort();
    for value in sorted {
        value.hash(hasher);
    }
}

/// Expands a task's input globs into the existing, non-ignored files they
/// match, sorted and deduplicated.
///
/// # Errors
///
/// Returns an error if an input glob is malformed.
fn matched_input_files(task: &Task) -> FyrerResult<Vec<PathBuf>> {
    let root =
        std::path::absolute(&task.project_root).unwrap_or_else(|_| task.project_root.clone());
    let ignore: Vec<glob::Pattern> = task
        .ignore
        .iter()
        .filter_map(|pattern| glob::Pattern::new(pattern).ok())
        .collect();
    let mut files = Vec::new();
    for pattern in &task.inputs {
        let full = root.join(pattern);
        let paths = glob::glob_with(&full.to_string_lossy(), MATCH_OPTIONS).map_err(|source| {
            FyrerError::Cache(CacheError::Glob {
                pattern: full.display().to_string(),
                source,
            })
        })?;
        for path in paths.flatten() {
            if !path.is_file() {
                continue;
            }
            let Ok(relative) = path.strip_prefix(&root) else {
                continue;
            };
            let ignored = ignore
                .iter()
                .any(|pattern| pattern.matches_path_with(relative, MATCH_OPTIONS));
            if !ignored {
                files.push(path);
            }
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn outputs_exist(task: &Task) -> bool {
    let root =
        std::path::absolute(&task.project_root).unwrap_or_else(|_| task.project_root.clone());
    task.outputs.iter().all(|pattern| {
        let full = root.join(pattern);
        match glob::glob_with(&full.to_string_lossy(), MATCH_OPTIONS) {
            Ok(paths) => paths.flatten().next().is_some(),
            Err(_) => false,
        }
    })
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::{compute_key, is_fresh_at, matched_input_files, record_at};
    use crate::{
        config::{RestartConfig, RestartStrategy},
        tasks::Task,
    };

    fn temp_dir(name: &str) -> std::path::PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "fyrer-cache-test-{}-{name}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn task(root: &std::path::Path, cmd: &str, inputs: &[&str], outputs: &[&str]) -> Task {
        Task {
            project_name: "proj".to_string(),
            project_root: root.to_path_buf(),
            env: HashMap::new(),
            task_name: "build".to_string(),
            cmd: cmd.to_string(),
            depends_on: vec![],
            inputs: inputs.iter().map(ToString::to_string).collect(),
            outputs: outputs.iter().map(ToString::to_string).collect(),
            ignore: vec![],
            cache: true,
            restart: RestartConfig {
                strategy: RestartStrategy::Never,
                delay: None,
            },
        }
    }

    fn write(path: &std::path::Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn key_is_deterministic() {
        let root = temp_dir("deterministic");
        write(&root.join("src/main.rs"), "one");
        let task = task(&root, "echo hi", &["src/**/*"], &["dist/**/*"]);
        assert_eq!(compute_key(&task).unwrap(), compute_key(&task).unwrap());
    }

    #[test]
    fn key_changes_with_input_content() {
        let root = temp_dir("input-content");
        let input = root.join("src/main.rs");
        write(&input, "one");
        let task = task(&root, "echo hi", &["src/**/*"], &["dist/**/*"]);
        let before = compute_key(&task).unwrap();
        write(&input, "two");
        assert_ne!(compute_key(&task).unwrap(), before);
    }

    #[test]
    fn key_changes_with_cmd_and_env() {
        let root = temp_dir("cmd-env");
        write(&root.join("src/main.rs"), "one");
        let base = task(&root, "echo hi", &["src/**/*"], &["dist/**/*"]);
        let key = compute_key(&base).unwrap();

        let mut different_cmd = base.clone();
        different_cmd.cmd = "echo bye".to_string();
        assert_ne!(compute_key(&different_cmd).unwrap(), key);

        let mut with_env = base.clone();
        with_env.env.insert("PORT".to_string(), "3000".to_string());
        assert_ne!(compute_key(&with_env).unwrap(), key);
    }

    #[test]
    fn fresh_requires_a_recorded_key() {
        let root = temp_dir("no-record");
        let cache = temp_dir("no-record-cache");
        write(&root.join("src/main.rs"), "one");
        write(&root.join("dist/out.js"), "built");
        let task = task(&root, "echo hi", &["src/**/*"], &["dist/**/*"]);
        assert!(!is_fresh_at(&cache, &task));
    }

    #[test]
    fn records_then_reports_fresh() {
        let root = temp_dir("recorded");
        let cache = temp_dir("recorded-cache");
        write(&root.join("src/main.rs"), "one");
        write(&root.join("dist/out.js"), "built");
        let task = task(&root, "echo hi", &["src/**/*"], &["dist/**/*"]);
        record_at(&cache, &task).unwrap();
        assert!(is_fresh_at(&cache, &task));
    }

    #[test]
    fn missing_outputs_invalidate_freshness() {
        let root = temp_dir("missing-outputs");
        let cache = temp_dir("missing-outputs-cache");
        write(&root.join("src/main.rs"), "one");
        write(&root.join("dist/out.js"), "built");
        let task = task(&root, "echo hi", &["src/**/*"], &["dist/**/*"]);
        record_at(&cache, &task).unwrap();
        std::fs::remove_file(root.join("dist/out.js")).unwrap();
        assert!(!is_fresh_at(&cache, &task));
    }

    #[test]
    fn changed_inputs_invalidate_freshness() {
        let root = temp_dir("changed-inputs");
        let cache = temp_dir("changed-inputs-cache");
        let input = root.join("src/main.rs");
        write(&input, "one");
        write(&root.join("dist/out.js"), "built");
        let task = task(&root, "echo hi", &["src/**/*"], &["dist/**/*"]);
        record_at(&cache, &task).unwrap();
        write(&input, "two");
        assert!(!is_fresh_at(&cache, &task));
    }

    #[test]
    fn ignored_inputs_are_not_hashed() {
        let root = temp_dir("ignored");
        let mut task = task(&root, "echo hi", &["**/*"], &["dist/**/*"]);
        task.ignore = vec!["*.log".to_string()];
        write(&root.join("a.txt"), "one");
        write(&root.join("server.log"), "logs");
        let files = matched_input_files(&task).unwrap();
        assert!(
            files
                .iter()
                .all(|path| path.extension() != Some("log".as_ref()))
        );
    }
}
