use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::Duration,
};

use anyhow::Result;
use tokio::{
    sync::broadcast::{self, Receiver, Sender},
    time::sleep,
};

use crate::{
    cache::{
        CacheProvider,
        local::{DEFAULT_CACHE_DIR, LocalCacheProvider},
    },
    config::{CacheConfig, FyrerConfig},
    events::AppEvent,
    executor::scheduler::Scheduler,
    task::{TaskGraph, TaskId, TaskMap, TaskState},
};

pub struct Orchestrator {
    // pub ui: Box<dyn Ui>,
    pub cache: Arc<dyn CacheProvider>,
    pub task_map: TaskMap,
    pub event_tx: Sender<AppEvent>,
    event_rx: Receiver<AppEvent>,
    tasks: HashMap<TaskId, TaskState>,
}

impl Orchestrator {
    pub fn new(config: FyrerConfig) -> Self {
        let cache = Self::cache_provider(&config.cache);
        let task_map = TaskMap::new(&config);
        let (event_tx, event_rx) = broadcast::channel(100);
        Orchestrator {
            // ui,
            task_map,
            cache,
            event_tx,
            event_rx,
            tasks: HashMap::new(),
        }
    }

    pub fn plan(&mut self, spec: Option<&str>) -> Result<()> {
        let graph = TaskGraph::new(self.task_map.clone())?;
        graph.validate()?;
        let tasks = self.task_map.get_tasks(spec)?;
        if tasks.is_empty() {
            return Err(anyhow::anyhow!("No tasks found for the given specifier"));
        }
        let levels = graph.get_orders(&tasks)?;
        println!("\nExecution plan:");
        for (i, level) in levels.iter().enumerate() {
            println!("  Level {} (parallel):", i + 1);
            for (j, task_id) in level.iter().enumerate() {
                let prefix = if j + 1 == level.len() {
                    "└──"
                } else {
                    "├──"
                };
                println!("    {prefix} {task_id}");
            }
            if i + 1 < levels.len() {
                println!("         ↓");
            }
        }
        println!();
        Ok(())
    }

    pub async fn run(&mut self, spec: Option<&str>) -> Result<()> {
        let graph = TaskGraph::new(self.task_map.clone())?;
        graph.validate()?;
        let tasks = self.task_map.get_tasks(spec)?;
        if tasks.is_empty() {
            return Err(anyhow::anyhow!("No tasks found for the given specifier"));
        }
        let levels = graph.get_orders(&tasks)?;
        let event_tx = self.event_tx.clone();
        let cache = self.cache.clone();
        let mut scheduler = Scheduler::new(self.task_map.clone(), levels, event_tx, cache);
        let mut scheduler_handle = tokio::spawn(async move { scheduler.run().await });
        loop {
            tokio::select! {
                result = &mut scheduler_handle => {
                    match result {
                        Ok(res) => {
                            println!("Scheduler finished with result: {:?}", res);
                            break;
                        }
                        Err(e) => {
                            eprintln!("Scheduler task panicked: {:?}", e);
                            break;
                        }
                    }
                }

                event = self.event_rx.recv() => {
                    match event {
                        Ok(app_event) => {
                            self.consume_event(app_event).await;
                        }
                        Err(broadcast::error::RecvError::Lagged(count)) => {}
                        Err(broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn consume_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::TaskSpawned {
                task_id,
                command_tx,
            } => {
                self.tasks.insert(
                    task_id.clone(),
                    TaskState {
                        command_tx: Some(command_tx),
                        status: crate::task::TaskStatus::Running,
                        logs: VecDeque::new(),
                    },
                );
            }
            AppEvent::KeyPress(key_event) => {
                if key_event.code == crossterm::event::KeyCode::Char('q') {
                    self.shutdown().await;
                }
            }
            AppEvent::Shutdown => {
                self.shutdown().await;
            }
            AppEvent::TaskLog {
                task_id,
                stream,
                line,
            } => {
                println!("[{}][{:?}] {}", task_id, stream, line);
            }

            _ => {}
        }
    }

    async fn shutdown(&mut self) {
        for (_, task_state) in &mut self.tasks {
            if let Some(command_tx) = &task_state.command_tx {
                let _ = command_tx.try_send(crate::events::TaskCommand::Kill);
            }
        }
        while self
            .tasks
            .values()
            .any(|s| s.status == crate::task::TaskStatus::Running)
        {
            sleep(Duration::from_millis(100)).await;
        }
    }

    fn listen_for_ctrl_c(&mut self) {
        let event_tx = self.event_tx.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.unwrap();
            let _ = event_tx.send(AppEvent::KeyPress(crossterm::event::KeyEvent::from(
                crossterm::event::KeyCode::Char('q'),
            )));
        });
    }
    fn cache_provider(cache_config: &CacheConfig) -> Arc<dyn CacheProvider> {
        match cache_config.provider {
            crate::config::CacheProviderKind::Local => {
                Arc::new(LocalCacheProvider::new(DEFAULT_CACHE_DIR.to_string()))
            }
        }
    }
}
