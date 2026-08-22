use std::{
    collections::{HashMap, HashSet},
    path::{PathBuf, Path},
    time::{Duration, SystemTime},
};

use fyrer_core::{TaskId, spec::TaskRegistry};
use tokio::sync::mpsc;

use fyrer_core::spec::TaskSpec;

/// Simple polling watcher — checks mtimes of inputs globs every `poll_interval`.
/// On change, sends `EngineCommand::Restart` for the affected task.
/// This avoids the `notify` crate for now and works cross-platform.

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
            // Map task -> last mtimes
            let mut last_mtimes: HashMap<TaskId, HashMap<PathBuf, SystemTime>> = HashMap::new();
            for (id, spec) in &watch_tasks {
                last_mtimes.insert(id.clone(), collect_mtimes(spec));
            }
            let mut pending: HashMap<TaskId, tokio::time::Instant> = HashMap::new();

            let mut interval = tokio::time::interval(self.poll_interval);
            loop {
                interval.tick().await;
                for (id, spec) in &watch_tasks {
                    let current = collect_mtimes(spec);
                    let last = last_mtimes.get(id).cloned().unwrap_or_default();
                    if current != last {
                        pending.insert(id.clone(), tokio::time::Instant::now() + self.debounce);
                        last_mtimes.insert(id.clone(), current);
                    }
                }
                // Check debounced pending
                let now = tokio::time::Instant::now();
                let mut to_restart = Vec::new();
                pending.retain(|id, deadline| {
                    if *deadline <= now {
                        to_restart.push(id.clone());
                        false
                    } else {
                        true
                    }
                });
                for id in to_restart {
                    eprintln!("[watch] file change detected for {}, restarting", id);
                    let _ = cmd_tx
                        .send(fyrer_engine::events::EngineCommand::Restart(vec![id]))
                        .await;
                }
            }
        })
    }
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
