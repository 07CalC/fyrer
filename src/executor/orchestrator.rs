use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use tokio::sync::broadcast::{self, Receiver, Sender};

use crate::{
    cache::{
        CacheProvider,
        local::{DEFAULT_CACHE_DIR, LocalCacheProvider},
    },
    config::{CacheConfig, FyrerConfig},
    events::AppEvent,
    executor::scheduler::Scheduler,
    task::{TaskGraph, TaskId, TaskMap, TaskState},
    ui::Ui,
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
        let summary = scheduler.run().await;
        println!("\nRun summary:");
        println!("  Total tasks: {}", summary.total);
        println!("  Successful: {}", summary.successful);
        println!("  Cached: {}", summary.cached);
        println!("  Failed: {}", summary.failed);
        println!("  Skipped: {}", summary.skipped);
        println!("  Duration: {:.2?}", summary.duration);
        Ok(())
    }
    fn cache_provider(cache_config: &CacheConfig) -> Arc<dyn CacheProvider> {
        match cache_config.provider {
            crate::config::CacheProviderKind::Local => {
                Arc::new(LocalCacheProvider::new(DEFAULT_CACHE_DIR.to_string()))
            }
        }
    }
}
