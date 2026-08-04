use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
    sync::Arc,
};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyModifiers};
use tokio::sync::mpsc::{self, Receiver, Sender, channel};

use crate::{
    FyrerConfig, TaskId,
    cli::Command,
    config::{TaskMap, TaskResolver},
    coordinator::run_coordinator,
    events::{AppEvent, TaskCommand},
    graph::TaskGraph,
    scheduler::{run_scheduler, spawn_ctrl_c_handler, spawn_input_collector},
    tasks::TaskStatus,
};

#[derive(Debug)]
pub struct App {
    pub should_quit: bool,
    pub task_map: Arc<TaskMap>,
    pub task_graph: TaskGraph,
    pub task_ids: Vec<TaskId>,
    pub task_status: HashMap<TaskId, TaskStatus>,
    pub event_bus_sender: Sender<AppEvent>,
    pub event_bus_receiver: Receiver<AppEvent>,
    pub running: HashMap<TaskId, Sender<TaskCommand>>,
    pub logs: HashMap<TaskId, Vec<String>>,
    pub pending_restarts: HashSet<TaskId>,
}

impl App {
    pub fn init(path: impl AsRef<Path>) -> Result<Self> {
        let config = FyrerConfig::new_from_path(path)?;
        let task_map = Arc::new(config.create_task_map()?);
        let task_graph = TaskGraph::new(&task_map)?;
        task_graph.validate()?;
        let (event_bus_sender, event_bus_receiver) = channel(task_map.len() * 10);
        Ok(Self {
            should_quit: false,
            task_map,
            task_graph,
            task_ids: Vec::new(),
            task_status: HashMap::new(),
            event_bus_sender,
            event_bus_receiver,
            running: HashMap::new(),
            logs: HashMap::new(),
            pending_restarts: HashSet::new(),
        })
    }

    pub async fn start(&mut self, command: Command) -> Result<()> {
        match command {
            Command::List => {
                self.list_tasks();
                Ok(())
            }
            Command::Run { task, dry_run } => {
                let task_ids = self.task_map.resolve(task.as_deref())?;
                let levels = self.task_graph.get_orders(&task_ids)?;
                let all_tasks_to_be_run: Vec<TaskId> = levels.iter().flatten().cloned().collect();
                self.task_ids = all_tasks_to_be_run;
                if dry_run {
                    for (mut i, batch) in levels.iter().enumerate() {
                        let task_names: Vec<String> =
                            batch.iter().map(|id| id.to_string()).collect();
                        i = i + 1;
                        println!("step {i}: {}", task_names.join(", "));
                    }
                    return Ok(());
                }
                self.run(levels).await?;
                Ok(())
            }
        }
    }

    async fn run(&mut self, levels: Vec<Vec<TaskId>>) -> Result<()> {
        let (event_bus_sender, mut event_bus_receiver) = mpsc::channel(self.task_map.len() * 10);
        spawn_input_collector(event_bus_sender.clone());
        spawn_ctrl_c_handler(event_bus_sender.clone());

        let task_map = self.task_map.clone();
        let scheduler_tx = event_bus_sender.clone();
        let all_task_ids: Vec<TaskId> = levels.iter().flatten().cloned().collect();
        tokio::spawn(async move { run_scheduler(levels.clone(), task_map, scheduler_tx).await });
        run_coordinator(
            all_task_ids,
            &self.task_map,
            event_bus_receiver,
            event_bus_sender,
        )
        .await?;
        Ok(())
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
            println!("Project: {}", project);
            for task in tasks {
                println!("  - {}", task);
            }
        }
    }
}
