use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use anyhow::Result;
use tokio::sync::mpsc;

use crate::{
    config::FyrerConfig,
    logs::process::ProcessLog,
    process::manager::ProcessManager,
    task::{TaskGraph, TaskId, TaskMap, TaskStatus},
};

pub struct FyrerEngine {
    task_map: TaskMap,
    task_graph: TaskGraph,
    processes: ProcessManager,
    status: HashMap<TaskId, TaskStatus>,
    log_tx: mpsc::Sender<ProcessLog>,
    log_rx: mpsc::Receiver<ProcessLog>,
}

impl FyrerEngine {
    pub fn new(config: FyrerConfig) -> Result<Self> {
        let task_map = TaskMap::new(&config);
        let task_graph = TaskGraph::new(task_map.clone())?;
        let processes = ProcessManager::new();
        let (log_tx, log_rx) = mpsc::channel(100);
        Ok(Self {
            task_map,
            task_graph,
            processes,
            status: HashMap::new(),
            log_tx,
            log_rx,
        })
    }

    pub async fn start(&mut self, spec: Option<&str>) -> Result<()> {
        self.task_graph.validate()?;
        let tasks = self.task_map.get_tasks(spec)?;
        let levels = self.task_graph.get_orders(&tasks)?;
        let start_time = Instant::now();

        for level in &levels {
            self.run_level(level).await;
        }

        self.print_summary(start_time.elapsed());
        Ok(())
    }

    async fn run_level(&mut self, level: &[TaskId]) {
        let mut pending = 0usize;
        for task_id in level {
            if self.is_blocked(task_id) {
                self.status.insert(task_id.clone(), TaskStatus::Skipped);
                println!("[{}] skipped", task_id);
                continue;
            }
            let task = self
                .task_map
                .get(task_id)
                .expect("task exists in the map");
            match self.processes.spawn(&task, self.log_tx.clone()) {
                Ok(()) => {
                    self.status.insert(task_id.clone(), TaskStatus::Running);
                    pending += 1;
                }
                Err(e) => {
                    self.status.insert(task_id.clone(), TaskStatus::Failed);
                    eprintln!("[{}] failed to spawn: {}", task_id, e);
                }
            }
        }

        while pending > 0 {
            match self.log_rx.recv().await {
                Some(ProcessLog::Exit { task_id, exit_code }) => {
                    self.record_exit(&task_id, exit_code);
                    pending -= 1;
                }
                Some(log) => Self::print_log(&log),
                None => break,
            }
        }
    }

    /// A task is blocked when any of its direct dependencies failed or was
    /// itself skipped, which propagates failures down the whole dependency chain.
    fn is_blocked(&self, task_id: &TaskId) -> bool {
        self.task_map.get(task_id).is_some_and(|task| {
            task.depends_on.iter().any(|dep| {
                matches!(
                    self.status.get(dep),
                    Some(TaskStatus::Failed | TaskStatus::Skipped)
                )
            })
        })
    }

    fn record_exit(&mut self, task_id: &TaskId, exit_code: i32) {
        let status = if exit_code == 0 {
            TaskStatus::Success
        } else {
            TaskStatus::Failed
        };
        self.status.insert(task_id.clone(), status);
    }

    fn print_log(log: &ProcessLog) {
        match log {
            ProcessLog::Stdout { task_id, data } => {
                println!("[{}] {}", task_id, String::from_utf8_lossy(data));
            }
            ProcessLog::Stderr { task_id, data } => {
                eprintln!("[{}] {}", task_id, String::from_utf8_lossy(data));
            }
            ProcessLog::System { task_id, data } => {
                eprintln!("[{}] {}", task_id, String::from_utf8_lossy(data));
            }
            ProcessLog::Exit { .. } => {}
        }
    }

    fn print_summary(&self, duration: Duration) {
        let total = self.status.len();
        let successful = self
            .status
            .values()
            .filter(|s| **s == TaskStatus::Success)
            .count();
        let failed = self
            .status
            .values()
            .filter(|s| **s == TaskStatus::Failed)
            .count();
        let skipped = self
            .status
            .values()
            .filter(|s| **s == TaskStatus::Skipped)
            .count();

        println!();
        println!("Run completed in {:.2?}", duration);
        println!("─────────────────────────");
        println!("  Successful: {}", successful);
        println!("  Failed: {}", failed);
        println!("  Skipped: {}", skipped);
        println!("  Total: {}", total);
    }
}