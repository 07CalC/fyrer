use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::{sync::broadcast::Sender, task::JoinSet};

use crate::{
    cache::CacheProvider,
    events::AppEvent,
    task::{ProcessResult, Task, TaskId, TaskMap, TaskStatus},
};

pub struct Scheduler {
    task_map: TaskMap,
    levels: Vec<Vec<TaskId>>,
    status: HashMap<TaskId, TaskStatus>,
    event_tx: Sender<AppEvent>,
    cache: Arc<dyn CacheProvider>,
}

#[derive(Debug, Clone)]
pub struct RunSummary {
    pub total: usize,
    pub successful: usize,
    pub cached: usize,
    pub failed: usize,
    pub skipped: usize,
    pub duration: Duration,
}

impl Scheduler {
    pub fn new(
        task_map: TaskMap,
        levels: Vec<Vec<TaskId>>,
        event_tx: Sender<AppEvent>,
        cache_provider: Arc<dyn CacheProvider>,
    ) -> Self {
        Self {
            task_map,
            levels,
            status: HashMap::new(),
            event_tx,
            cache: cache_provider,
        }
    }
    pub async fn run(&mut self) -> RunSummary {
        let start_time = Instant::now();
        let levels = std::mem::take(&mut self.levels);
        for level in &levels {
            self.run_level(level).await;
        }
        let _ = self.event_tx.send(AppEvent::RunFinished);
        RunSummary {
            total: self.status.len(),
            successful: self
                .status
                .values()
                .filter(|s| **s == TaskStatus::Success)
                .count(),
            cached: self
                .status
                .values()
                .filter(|s| **s == TaskStatus::Cached)
                .count(),
            failed: self
                .status
                .values()
                .filter(|s| **s == TaskStatus::Failed)
                .count(),
            skipped: self
                .status
                .values()
                .filter(|s| **s == TaskStatus::Skipped)
                .count(),
            duration: start_time.elapsed(),
        }
    }

    async fn run_level(&mut self, level: &[TaskId]) {
        let mut processes = JoinSet::new();
        let event_tx = self.event_tx.clone();
        for task_id in level {
            if self.is_blocked(task_id) {
                self.status.insert(task_id.clone(), TaskStatus::Skipped);
                let _ = event_tx.send(AppEvent::TaskSkipped {
                    task_id: task_id.clone(),
                });
                continue;
            }
            let task = self
                .task_map
                .get(task_id)
                .expect("task id exists in the graph");
            if self.cache_hit(&task) {
                self.status.insert(task_id.clone(), TaskStatus::Cached);
                let _ = event_tx.send(AppEvent::TaskCacheHit {
                    task_id: task_id.clone(),
                });

                continue;
            }
            match task.spawn(event_tx.clone()) {
                Ok(process) => {
                    let _ = event_tx.send(AppEvent::TaskSpawned {
                        task_id: task_id.clone(),
                        command_tx: process.command_tx.clone(),
                    });
                    let task_id = task_id.clone();
                    processes.spawn(async move {
                        let result = process.handle.await;
                        (task_id, result)
                    });
                }
                Err(e) => {
                    self.status.insert(task_id.clone(), TaskStatus::Failed);
                    let _ = event_tx.send(AppEvent::TaskFailed {
                        task_id: task_id.clone(),
                        exit_code: -1,
                        error: Some(e.to_string()),
                    });
                }
            }
        }
        while let Some(result) = processes.join_next().await {
            match result {
                Ok((task_id, Ok(ProcessResult::Success { .. }))) => {
                    self.status.insert(task_id, TaskStatus::Success);
                }

                Ok((task_id, Ok(ProcessResult::Failure { .. }))) => {
                    self.status.insert(task_id, TaskStatus::Failed);
                }

                Ok((task_id, Err(e))) => {
                    self.status.insert(task_id.clone(), TaskStatus::Failed);

                    let _ = event_tx.send(AppEvent::TaskFailed {
                        task_id,
                        exit_code: -1,
                        error: Some(e.to_string()),
                    });
                }

                // the joinset itself panicked, which is unexpected, but we can log it and continue
                Err(e) => {
                    eprintln!("process task panicked: {e}");
                }
            }
        }
    }
    //TODO: all error handling should be explicitly done later, for now we just treat any error as a
    //cache miss and run the task

    fn cache_hit(&self, task: &Task) -> bool {
        if !task.cache {
            return false;
        }
        let cache_key = match task.cache_key(self.task_map.clone()) {
            Ok(key) => key,
            Err(_) => {
                return false;
            }
        };
        if !self.cache.contains(&cache_key) {
            return false;
        }
        let output_digest = match task.output_digest() {
            Ok(key) => key,
            Err(_) => {
                return false;
            }
        };
        match self.cache.need_hydration(&output_digest) {
            // outputs are already in place and intact, nothing to do
            Ok(false) => true,
            // outputs are missing or corrupted, need to restore from cache
            Ok(true) => self.cache.restore(&cache_key).unwrap_or(false),
            // failed to check if hydration is needed, treat as cache miss
            Err(_) => false,
        }
    }

    /// a task is blocked when any of its direct dependencies failed or was itself
    /// skipped, which propagates failures down the whole dependency chain.
    fn is_blocked(&self, task_id: &TaskId) -> bool {
        if let Some(task) = self.task_map.get(task_id) {
            for dep_id in &task.depends_on {
                if let Some(status) = self.status.get(dep_id) {
                    if matches!(status, TaskStatus::Failed | TaskStatus::Skipped) {
                        return true;
                    }
                }
            }
        }
        false
    }
}
