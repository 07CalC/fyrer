use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
    sync::Arc,
};

use anyhow::Result;
use tokio::sync::mpsc::{Receiver, Sender, channel};

use crate::{
    FyrerConfig, TaskId,
    cli::Command,
    events::{AppEvent, TaskCommand},
    graph::TaskGraph,
    tasks::{TaskMap, TaskStatus},
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
    pub task_command_senders: HashMap<TaskId, Sender<TaskCommand>>,
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
            task_command_senders: HashMap::new(),
            logs: HashMap::new(),
            pending_restarts: HashSet::new(),
        })
    }

    pub fn start(&mut self, command: Command) -> Result<()> {
        match command {
            Command::List => {
                self.list_tasks();
                Ok(())
            }
            Command::Run { task, dry_run } => Ok(()),
        }
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
