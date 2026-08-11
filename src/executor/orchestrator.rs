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
        println!("Execution plan:");
        for (i, level) in levels.iter().enumerate() {
            println!("Step: {}", i + 1);
            for task_id in level {
                println!("  - {}", task_id);
            }
        }
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
