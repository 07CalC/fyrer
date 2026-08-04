use std::{
    collections::HashMap,
    fmt,
    hash::{DefaultHasher, Hash, Hasher},
    path::PathBuf,
};

use anyhow::{Result, anyhow};

use crate::config::RestartConfig;
use crate::env::EnvMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskId {
    project_name: String,
    task_name: String,
}

#[derive(Debug)]
pub enum TaskStatus {
    Waiting,
    Complete,
    Failed(i32, Option<String>),
    Exited(i32),
    Restarting,
}

#[derive(Debug)]
pub struct Task {
    pub project_name: String,
    pub project_root: PathBuf,
    pub env: EnvMap,
    pub task_name: String,
    pub cmd: String,
    pub depends_on: Vec<String>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub ignore: Vec<String>,
    pub cache: bool,
    pub restart: RestartConfig,
    // pub status: TaskStatus,
    // pub task_channel: (Sender<TaskChannelMessage>, Receiver<TaskChannelMessage>),
    // pub process_channel: (
    //     Sender<ProcessChannelMessage>,
    //     Receiver<ProcessChannelMessage>,
    // ),
}

pub type TaskMap = HashMap<TaskId, Task>;

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.project_name, self.task_name)
    }
}

impl TaskId {
    #[must_use]
    pub fn new(project_name: &str, task_name: &str) -> Self {
        Self {
            project_name: project_name.to_string(),
            task_name: task_name.to_string(),
        }
    }

    #[must_use]
    pub fn project_name(&self) -> &str {
        &self.project_name
    }
    #[must_use]
    pub fn task_name(&self) -> &str {
        &self.task_name
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let (project, task) = s.split_once(':')?;
        if project.is_empty() || task.is_empty() || task.contains(':') {
            return None;
        }
        Some(Self::new(project, task))
    }

    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn hash(&self) -> usize {
        let mut hasher = DefaultHasher::new();
        Hash::hash(self, &mut hasher);
        hasher.finish() as usize
    }
}
