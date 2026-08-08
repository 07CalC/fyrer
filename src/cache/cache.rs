use crate::{Task, TaskId, config::TaskMap};
use anyhow::{Result, anyhow, bail};
use glob::glob;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};
pub fn get_hash(task: &Task, task_map: &TaskMap) -> Result<String> {
    let start = std::time::Instant::now();
    let mut memo = HashMap::new();
    let mut visiting = HashSet::new();
    let hash = task_hash(&task.id(), task_map, &mut memo, &mut visiting);
    println!(
        "Computed hash for task '{}' in {:?}",
        task.id(),
        start.elapsed()
    );
    hash
}

fn task_hash(
    task_id: &TaskId,
    task_map: &TaskMap,
    memo: &mut HashMap<TaskId, String>,
    visiting: &mut HashSet<TaskId>,
) -> Result<String> {
    if let Some(hash) = memo.get(task_id) {
        return Ok(hash.clone());
    }
    if !visiting.insert(task_id.clone()) {
        bail!("Circular dependency detected involving task '{}'", task_id);
    }

    let task = task_map
        .get(task_id)
        .ok_or_else(|| anyhow!("Task '{}' not found in task map", task_id))?;

    let mut hasher = blake3::Hasher::new();
    hash_kv(&mut hasher, b"id", task_id.to_string().as_bytes());
    hash_kv(&mut hasher, b"cmd", task.cmd.as_bytes());
    hash_kv(
        &mut hasher,
        b"root",
        task.project_root.to_string_lossy().as_bytes(),
    );

    let mut dep_ids = Vec::new();
    let mut seen_deps = HashSet::new();
    for spec in &task.depends_on {
        let dep_id = TaskId::parse(spec)
            .ok_or_else(|| anyhow!("Invalid dependency '{}' of task '{}'", spec, task_id))?;
        if !task_map.contains_key(&dep_id) {
            return Err(anyhow!(
                "Dependency '{}' of task '{}' not found",
                spec,
                task_id
            ));
        }
        if seen_deps.insert(dep_id.clone()) {
            dep_ids.push(dep_id);
        }
    }
    dep_ids.sort_by_key(ToString::to_string);
    for dep_id in &dep_ids {
        let dep_hash = task_hash(dep_id, task_map, memo, visiting)?;
        hash_kv(&mut hasher, b"dep", dep_hash.as_bytes());
    }

    let mut inputs: Vec<String> = task.inputs.clone();
    inputs.sort();
    for input in &inputs {
        hash_kv(&mut hasher, b"input", input.as_bytes());
    }

    let mut outputs: Vec<String> = task.outputs.clone();
    outputs.sort();
    for output in &outputs {
        hash_kv(&mut hasher, b"output", output.as_bytes());
    }

    let mut ignore: Vec<String> = task.ignore.clone();
    ignore.sort();
    for pattern in &ignore {
        hash_kv(&mut hasher, b"ignore", pattern.as_bytes());
    }

    let mut env: Vec<(&String, &String)> = task.env.iter().collect();
    env.sort();
    for (key, value) in env {
        hash_kv(&mut hasher, b"env_key", key.as_bytes());
        hash_kv(&mut hasher, b"env_val", value.as_bytes());
    }

    for input in &inputs {
        hash_input_files(&mut hasher, task, input)?;
    }

    visiting.remove(task_id);
    let hash = hasher.finalize().to_hex().to_string();
    memo.insert(task_id.clone(), hash.clone());
    Ok(hash)
}

fn hash_input_files(hasher: &mut blake3::Hasher, task: &Task, input: &str) -> Result<()> {
    let pattern = task.project_root.join(input);
    let pattern_str = pattern.to_string_lossy();
    let entries =
        glob(&pattern_str).map_err(|e| anyhow!("Invalid input glob '{}': {}", pattern_str, e))?;
    let mut matched: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let path =
            entry.map_err(|e| anyhow!("Error walking input glob '{}': {}", pattern_str, e))?;
        if !path.is_file() {
            continue;
        }
        let rel = path.strip_prefix(&task.project_root).unwrap_or(&path);
        if is_ignored(rel, &task.ignore) {
            continue;
        }
        matched.push(path);
    }
    matched.sort();

    hash_kv(hasher, b"file_count", &(matched.len() as u64).to_le_bytes());
    if matched.is_empty() {
        return Err(anyhow!(
            "No files matched input '{}' of task '{}'",
            input,
            task.id()
        ));
    }
    for path in &matched {
        let rel = path.strip_prefix(&task.project_root).unwrap_or(path);
        hash_kv(hasher, b"path", rel.to_string_lossy().as_bytes());
        let content = fs::read(path)?;
        hash_kv(hasher, b"content", &content);
    }
    Ok(())
}

pub fn hash_output_files(task: &Task) -> Result<String> {
    let mut hasher = blake3::Hasher::new();
    let mut matched: Vec<PathBuf> = Vec::new();
    for output in &task.outputs {
        let pattern = task.project_root.join(output);
        let pattern_str = pattern.to_string_lossy();
        let entries = glob(&pattern_str)
            .map_err(|e| anyhow!("Invalid output glob '{}': {}", pattern_str, e))?;
        for entry in entries {
            let path =
                entry.map_err(|e| anyhow!("Error walking output glob '{}': {}", pattern_str, e))?;
            if !path.is_file() {
                continue;
            }
            let rel = path.strip_prefix(&task.project_root).unwrap_or(&path);
            if is_ignored(rel, &task.ignore) {
                continue;
            }
            matched.push(path);
        }
    }
    matched.sort();

    hash_kv(
        &mut hasher,
        b"file_count",
        &(matched.len() as u64).to_le_bytes(),
    );
    for path in &matched {
        let rel = path.strip_prefix(&task.project_root).unwrap_or(path);
        hash_kv(&mut hasher, b"path", rel.to_string_lossy().as_bytes());
        let content = fs::read(path)?;
        hash_kv(&mut hasher, b"content", &content);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn is_ignored(rel_path: &Path, patterns: &[String]) -> bool {
    patterns
        .iter()
        .any(|p| glob::Pattern::new(p).is_ok_and(|pat| pat.matches_path(rel_path)))
}

fn hash_kv(hasher: &mut blake3::Hasher, key: &[u8], value: &[u8]) {
    hasher.update(&(key.len() as u64).to_le_bytes());
    hasher.update(key);
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::RestartConfig, env::EnvMap};

    fn make_task(id: &str, env: EnvMap, depends_on: Vec<String>) -> Task {
        Task {
            project_name: "m".to_string(),
            project_root: PathBuf::from("."),
            env,
            task_name: id.to_string(),
            cmd: "echo hi".to_string(),
            depends_on,
            inputs: Vec::new(),
            outputs: Vec::new(),
            ignore: Vec::new(),
            cache: true,
            restart: RestartConfig::default(),
        }
    }

    fn make_map(tasks: Vec<Task>) -> TaskMap {
        tasks.into_iter().map(|t| (t.id(), t)).collect::<TaskMap>()
    }

    #[test]
    fn hash_is_stable_regardless_of_env_order() {
        let map = make_map(vec![
            make_task(
                "a",
                EnvMap::from([
                    ("X".to_string(), "1".to_string()),
                    ("Y".to_string(), "2".to_string()),
                ]),
                Vec::new(),
            ),
            make_task(
                "b",
                EnvMap::from([
                    ("Y".to_string(), "2".to_string()),
                    ("X".to_string(), "1".to_string()),
                ]),
                vec!["m:a".to_string()],
            ),
        ]);
        let id = TaskId::new("m", "b");
        let first = get_hash(&map[&id], &map).unwrap();
        let second = get_hash(&map[&id], &map).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn hash_bubbles_up_dependency_changes() {
        let mut map = make_map(vec![
            make_task("a", EnvMap::new(), Vec::new()),
            make_task("b", EnvMap::new(), vec!["m:a".to_string()]),
        ]);
        let id_b = TaskId::new("m", "b");
        let before = get_hash(&map[&id_b], &map).unwrap();

        map.get_mut(&TaskId::new("m", "a")).unwrap().cmd = "echo changed".to_string();
        let after = get_hash(&map[&id_b], &map).unwrap();
        assert_ne!(before, after);
    }

    #[test]
    fn hash_errors_on_circular_dependency() {
        let map = make_map(vec![
            make_task("a", EnvMap::new(), vec!["m:b".to_string()]),
            make_task("b", EnvMap::new(), vec!["m:a".to_string()]),
        ]);
        assert!(get_hash(&map[&TaskId::new("m", "a")], &map).is_err());
    }

    #[test]
    fn hash_errors_on_missing_dependency() {
        let map = make_map(vec![make_task(
            "a",
            EnvMap::new(),
            vec!["m:nope".to_string()],
        )]);
        assert!(get_hash(&map[&TaskId::new("m", "a")], &map).is_err());
    }
}
