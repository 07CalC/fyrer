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
    should_quit: bool,
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
            should_quit: false,
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
        let mut event_tx = self.event_tx.clone();
        tokio::spawn(async move {
            sleep(Duration::from_secs(5)).await;
            print!("sending shutdown signal");
            let _ = event_tx.send(AppEvent::Shutdown);
        });
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
                            if matches!(app_event, AppEvent::Shutdown) {
                                println!("Shutdown signal received. Terminating tasks...");
                                self.shutdown().await;
                                println!("All tasks terminated. Exiting.");
                            }
                            let quit = self.consume_event(app_event).await;
                            if quit {
                                println!("Exiting application.");
                                break;
                            }
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

    async fn consume_event(&mut self, event: AppEvent) -> bool {
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
                false
            }
            AppEvent::KeyPress(key_event) => {
                if key_event.code == crossterm::event::KeyCode::Char('q') {
                    let _ = self.event_tx.send(AppEvent::Shutdown);
                }
                false
            }
            AppEvent::TaskLog {
                task_id,
                stream,
                line,
            } => {
                println!("[{}][{:?}] {}", task_id, stream, line);
                false
            }
            AppEvent::TaskComplete { task_id } => {
                if let Some(task_state) = self.tasks.get_mut(&task_id) {
                    task_state.status = crate::task::TaskStatus::Success;
                }
                self.safe_to_quit()
            }
            AppEvent::TaskFailed {
                task_id,
                exit_code,
                error,
            } => {
                if let Some(task_state) = self.tasks.get_mut(&task_id) {
                    task_state.status = crate::task::TaskStatus::Failed;
                }
                eprintln!(
                    "Task {} failed with exit code {}: {:?}",
                    task_id, exit_code, error
                );
                self.safe_to_quit()
            }

            _ => false,
        }
    }

    async fn shutdown(&mut self) {
        for (_, task_state) in &mut self.tasks {
            if let Some(command_tx) = &task_state.command_tx {
                let _ = command_tx.try_send(crate::events::TaskCommand::Kill);
            }
        }
        self.should_quit = true;
    }

    fn safe_to_quit(&self) -> bool {
        !self
            .tasks
            .values()
            .any(|task_state| matches!(task_state.status, crate::task::TaskStatus::Running))
            && self.should_quit
    }

    fn listen_for_ctrl_c(&mut self) {
        let event_tx = self.event_tx.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.unwrap();
            let _ = event_tx.send(AppEvent::Shutdown);
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
