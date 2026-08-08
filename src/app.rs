use std::{collections::BTreeMap, path::Path, sync::Arc};

use anyhow::Result;

use crate::{
    FyrerConfig, TaskId,
    cache::{CacheProvider, build_cache_provider},
    cli::Command,
    config::{TaskMap, TaskResolver},
    graph::TaskGraph,
    orchestrator,
    tui::{PlainUi, Tui, Ui},
};

pub struct App {
    pub task_map: Arc<TaskMap>,
    pub task_graph: TaskGraph,
    /// Shared cache backend, built once from config and passed through the
    /// entire run pipeline.
    pub cache_provider: Arc<dyn CacheProvider>,
}

impl App {
    /// Loads configuration and builds the task graph.
    ///
    /// # Errors
    ///
    /// Returns an error if the config file cannot be read, parsed, or the
    /// resulting task graph fails validation.
    pub fn init(path: impl AsRef<Path>) -> Result<Self> {
        let config = FyrerConfig::new_from_path(path)?;
        let task_map = Arc::new(config.create_task_map()?);
        let task_graph = TaskGraph::new(&task_map)?;
        task_graph.validate()?;
        let cache_provider = build_cache_provider(&config.cache.provider);
        Ok(Self {
            task_map,
            task_graph,
            cache_provider,
        })
    }

    /// Dispatches the parsed CLI command.
    ///
    /// # Errors
    ///
    /// Returns an error if task resolution, graph ordering, or the run itself
    /// fails.
    pub async fn start(&mut self, command: Command) -> Result<()> {
        match command {
            Command::List => {
                self.list_tasks();
                Ok(())
            }
            Command::Run {
                task,
                dry_run,
                no_tui,
            } => {
                let start_time = std::time::Instant::now();
                let task_ids = self.task_map.resolve(task.as_deref())?;
                let levels = self.task_graph.get_orders(&task_ids)?;
                if dry_run {
                    for (i, batch) in levels.iter().enumerate() {
                        let task_names: Vec<String> =
                            batch.iter().map(|(id, _)| id.to_string()).collect();
                        println!("step {}: {}", i + 1, task_names.join(", "));
                    }
                    return Ok(());
                }
                self.run(levels, no_tui).await?;
                let elapsed = start_time.elapsed();
                println!("All tasks completed in {:.2?}", elapsed);
                Ok(())
            }
        }
    }

    async fn run(&self, levels: Vec<Vec<(TaskId, Vec<TaskId>)>>, no_tui: bool) -> Result<()> {
        let ui: Box<dyn Ui> = if no_tui {
            Box::new(PlainUi::default())
        } else {
            Box::new(Tui::new()?)
        };
        // The TUI keeps running so the user can inspect results; non-TUI
        // output exits as soon as all tasks have finished.
        orchestrator::run(
            levels,
            self.task_map.clone(),
            self.cache_provider.clone(),
            ui,
            no_tui,
        )
        .await
    }

    fn list_tasks(&self) {
        let mut projects: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for task in self.task_map.values() {
            projects
                .entry(&task.project_name)
                .or_default()
                .push(&task.task_name);
        }
        for (project, mut tasks) in projects {
            tasks.sort_unstable();
            println!("Project: {project}");
            for task in tasks {
                println!("  - {task}");
            }
        }
    }
}
