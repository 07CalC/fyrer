use anyhow::Result;
use owo_colors::OwoColorize;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::broadcast::{self, Sender},
    task::JoinHandle,
    time,
};

use crate::{
    cache::{
        CacheProvider,
        local::{DEFAULT_CACHE_DIR, LocalCacheProvider},
    },
    config::{CacheConfig, FyrerConfig},
    events::{AppEvent, TaskCommand},
    executor::scheduler::Scheduler,
    logs::error_collector::ErrorCollector,
    task::{TaskGraph, TaskId, TaskMap, TaskState},
    ui::Ui,
};

pub struct Orchestrator {
    pub cache: Arc<dyn CacheProvider>,
    pub task_map: TaskMap,
    pub event_tx: Sender<AppEvent>,
    tasks: HashMap<TaskId, TaskState>,
}

impl Orchestrator {
    pub fn new(config: FyrerConfig) -> Self {
        let cache = Self::cache_provider(&config.cache);
        let task_map = TaskMap::new(&config);
        let (event_tx, _) = broadcast::channel(512);
        Orchestrator {
            task_map,
            cache,
            event_tx,
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

    pub async fn run<U: Ui>(&mut self, spec: Option<&str>, ui: U) -> Result<()> {
        let graph = TaskGraph::new(self.task_map.clone())?;
        graph.validate()?;
        let tasks = self.task_map.get_tasks(spec)?;
        if tasks.is_empty() {
            return Err(anyhow::anyhow!("No tasks found for the given specifier"));
        }
        let levels = graph.get_orders(&tasks)?;
        let ui_rx = self.event_tx.subscribe();
        let mut orch_rx = self.event_tx.subscribe();

        let mut ui_handle: JoinHandle<Result<()>> = ui.start(ui_rx);

        let event_tx = self.event_tx.clone();
        let cache = self.cache.clone();
        let mut scheduler = Scheduler::new(self.task_map.clone(), levels, event_tx.clone(), cache);
        let scheduler_handle = tokio::spawn(async move { scheduler.run().await });

        self.listen_for_ctrl_c();

        let stop = Arc::new(AtomicBool::new(false));
        let (input_handle, tick_handle) = self.start_input_capture(Arc::clone(&stop));
        let mut error_collector = ErrorCollector::new();

        loop {
            tokio::select! {
                msg = orch_rx.recv() => {
                    match msg {
                        Ok(AppEvent::Shutdown) => {
                            self.kill_all_tasks();
                            break;
                        }
                        Ok(AppEvent::TaskSpawned { task_id, command_tx }) => {
                            self.tasks.insert(
                                task_id,
                                TaskState {
                                    command_tx: Some(command_tx),
                                    status: crate::task::TaskStatus::Running,
                                },
                            );
                        }
                        Ok(AppEvent::TaskComplete { task_id }) => {
                            if let Some(s) = self.tasks.get_mut(&task_id) {
                                s.status = crate::task::TaskStatus::Success;
                            }
                        }
                        Ok(AppEvent::TaskFailed { task_id, .. }) => {
                            if let Some(s) = self.tasks.get_mut(&task_id) {
                                s.status = crate::task::TaskStatus::Failed;
                            }
                        }
                        Ok(AppEvent::NonFatalError { task_id, error }) => {
                            error_collector.push_error(task_id, error);
                        }
                        Ok(AppEvent::Warning{ task_id, message }) => {
                            error_collector.push_warning(task_id, message);
                        }
                        Ok(AppEvent::RunFinished(_)) => {}
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                _ = &mut ui_handle => {
                    self.kill_all_tasks();
                    break;
                }
            }
        }

        stop.store(true, Ordering::Relaxed);
        tick_handle.abort();
        let _ = input_handle.await;
        let _ = tick_handle.await;
        let result = scheduler_handle.await;
        match result {
            Ok(r) => {
                println!();
                println!(
                    "{} {}",
                    "Run completed in".bold(),
                    format!("{:.2?}", r.duration).dimmed()
                );
                println!();

                println!("  {}", "Results".bold());
                println!("  {}", "─────────────────────────".dimmed());

                println!(
                    "  {} {:<12} {}",
                    "+".green().bold(),
                    "Successful",
                    r.successful.to_string().green()
                );

                println!(
                    "  {} {:<12} {}",
                    "x".red().bold(),
                    "Failed",
                    r.failed.to_string().red()
                );

                println!(
                    "  {} {:<12} {}",
                    "*".cyan().bold(),
                    "Cached",
                    r.cached.to_string().cyan()
                );

                println!(
                    "  {} {:<12} {}",
                    "-".yellow().bold(),
                    "Skipped",
                    r.skipped.to_string().yellow()
                );

                println!("  {:<14} {}", "Total".bold(), r.total.to_string().bold());
                if r.cached == r.total && r.total > 0 {
                    println!();
                    println!("  {}", "ALL CACHED".cyan().bold());
                    println!("  {}", "FYRER FIRED.".cyan().bold());
                    println!();
                }
            }
            Err(_) => {}
        }
        error_collector.finalize();
        Ok(())
    }

    fn kill_all_tasks(&mut self) {
        for task_state in self.tasks.values() {
            if let Some(tx) = &task_state.command_tx {
                let _ = tx.try_send(TaskCommand::Kill);
            }
        }
    }

    fn listen_for_ctrl_c(&mut self) {
        let event_tx = self.event_tx.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.unwrap();
            let _ = event_tx.send(AppEvent::Shutdown);
        });
    }

    fn start_input_capture(&self, stop: Arc<AtomicBool>) -> (JoinHandle<()>, JoinHandle<()>) {
        let input_tx = self.event_tx.clone();
        let tick_tx = self.event_tx.clone();

        let input_handle = tokio::task::spawn_blocking(move || {
            loop {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                if !crossterm::event::poll(Duration::from_millis(50)).unwrap_or(false) {
                    continue;
                }
                match crossterm::event::read() {
                    Ok(crossterm::event::Event::Key(key)) => {
                        if input_tx.send(AppEvent::KeyPress(key)).is_err() {
                            break;
                        }
                    }
                    Ok(crossterm::event::Event::Mouse(mouse)) => {
                        let dir = match mouse.kind {
                            crossterm::event::MouseEventKind::ScrollUp => {
                                Some(crate::events::ScrollDirection::Up)
                            }
                            crossterm::event::MouseEventKind::ScrollDown => {
                                Some(crate::events::ScrollDirection::Down)
                            }
                            _ => None,
                        };
                        if let Some(d) = dir {
                            if input_tx.send(AppEvent::MouseScroll(d)).is_err() {
                                break;
                            }
                        }
                    }
                    Err(_) => break,
                    _ => {}
                }
            }
        });

        let tick_handle = tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_millis(100));
            loop {
                interval.tick().await;
                if tick_tx.send(AppEvent::Tick).is_err() {
                    break;
                }
            }
        });

        (input_handle, tick_handle)
    }

    fn cache_provider(cache_config: &CacheConfig) -> Arc<dyn CacheProvider> {
        match cache_config.provider {
            crate::config::CacheProviderKind::Local => {
                Arc::new(LocalCacheProvider::new(DEFAULT_CACHE_DIR.to_string()))
            }
        }
    }
}
