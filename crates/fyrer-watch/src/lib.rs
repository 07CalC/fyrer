//! Polling file watcher: watches `watch: true` tasks' input globs and sends
//! [`fyrer_engine::events::EngineCommand::FilesChanged`] on changes, with
//! debouncing. Polling (rather than inotify) keeps it cross-platform and
//! dependency-free; mtimes of input files are compared every poll interval.

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::{Duration, SystemTime},
};

use fyrer_core::{
    TaskId,
    spec::{TaskRegistry, TaskSpec},
};
use tokio::sync::mpsc;

/// Simple polling watcher — checks mtimes of inputs globs every `poll_interval`.
/// On change, sends `EngineCommand::FilesChanged` (with the changed paths) for
/// the affected task after the debounce window.

pub struct Watcher {
    registry: TaskRegistry,
    poll_interval: Duration,
    debounce: Duration,
}

impl Watcher {
    pub fn new(registry: TaskRegistry) -> Self {
        Self {
            registry,
            poll_interval: Duration::from_millis(300),
            debounce: Duration::from_millis(300),
        }
    }

    pub fn with_intervals(mut self, poll: Duration, debounce: Duration) -> Self {
        self.poll_interval = poll;
        self.debounce = debounce;
        self
    }

    /// Spawn a background task that watches all `watch=true` tasks.
    /// Returns a JoinHandle that can be aborted when the run finishes.
    pub fn spawn(
        self,
        cmd_tx: mpsc::Sender<fyrer_engine::events::EngineCommand>,
    ) -> tokio::task::JoinHandle<()> {
        // Collect watch tasks and their input globs
        let watch_tasks: Vec<(TaskId, TaskSpec)> = self
            .registry
            .iter()
            .filter(|(_, s)| s.watch)
            .map(|(id, s)| (id.clone(), (**s).clone()))
            .collect();

        if watch_tasks.is_empty() {
            // No watch tasks — return a handle that immediately completes
            return tokio::spawn(async {});
        }

        tokio::spawn(async move {
            // task -> mtime snapshot of its input files
            let mut last_mtimes: HashMap<TaskId, HashMap<PathBuf, SystemTime>> = HashMap::new();
            for (id, spec) in &watch_tasks {
                last_mtimes.insert(id.clone(), collect_mtimes(spec));
            }
            // Debounce queue: task -> (fire-at instant, changed files).
            let mut pending: HashMap<TaskId, (tokio::time::Instant, Vec<PathBuf>)> = HashMap::new();

            let mut interval = tokio::time::interval(self.poll_interval);
            loop {
                interval.tick().await;
                for (id, spec) in &watch_tasks {
                    let current = collect_mtimes(spec);
                    let last = last_mtimes.get(id).cloned().unwrap_or_default();
                    if current != last {
                        let changed = changed_paths(&last, &current);
                        last_mtimes.insert(id.clone(), current);
                        let entry = pending
                            .entry(id.clone())
                            .or_insert_with(|| (tokio::time::Instant::now() + self.debounce, Vec::new()));
                        entry.1.extend(changed);
                    }
                }
                // Fire debounced changes.
                let now = tokio::time::Instant::now();
                let due: Vec<TaskId> = pending
                    .iter()
                    .filter(|(_, (deadline, _))| *deadline <= now)
                    .map(|(id, _)| id.clone())
                    .collect();
                for id in due {
                    if let Some((_, paths)) = pending.remove(&id) {
                        let _ = cmd_tx
                            .send(fyrer_engine::events::EngineCommand::FilesChanged(id, paths))
                            .await;
                    }
                }
            }
        })
    }
}

/// Files whose mtime appeared, disappeared or moved forward between the two
/// snapshots.
fn changed_paths(
    last: &HashMap<PathBuf, SystemTime>,
    current: &HashMap<PathBuf, SystemTime>,
) -> Vec<PathBuf> {
    let mut changed = Vec::new();
    for (path, mtime) in current {
        match last.get(path) {
            Some(prev) if prev == mtime => {}
            _ => changed.push(path.clone()),
        }
    }
    for path in last.keys() {
        if !current.contains_key(path) {
            changed.push(path.clone());
        }
    }
    changed
}

fn collect_mtimes(spec: &TaskSpec) -> HashMap<PathBuf, SystemTime> {
    let mut out = HashMap::new();
    let ignore = collect_ignore(spec);
    for pat in &spec.inputs {
        let base_pat = spec.cwd.join(pat).to_string_lossy().to_string();
        // Handle `/**` which in Rust glob needs `/**/*` to match files
        let mut pats = vec![base_pat.clone()];
        if base_pat.ends_with("/**") {
            pats.push(format!("{}/{}", base_pat, "*"));
            pats.push(format!("{}/{}", base_pat, "**/*"));
        }
        for glob_pat in pats {
            if let Ok(paths) = glob::glob(&glob_pat) {
                for p in paths.flatten() {
                    if ignore.contains(&p) {
                        continue;
                    }
                    if let Ok(meta) = std::fs::metadata(&p) {
                        if meta.is_file() {
                            if let Ok(mtime) = meta.modified() {
                                out.insert(p, mtime);
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

fn collect_ignore(spec: &TaskSpec) -> HashSet<PathBuf> {
    let mut set = HashSet::new();
    for pat in &spec.ignore {
        let glob_pat = spec.cwd.join(pat).to_string_lossy().to_string();
        if let Ok(paths) = glob::glob(&glob_pat) {
            for p in paths.flatten() {
                set.insert(p);
            }
        }
    }
    set
}
