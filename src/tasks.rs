use crate::config::RestartConfig;
use crate::env::EnvMap;
use std::fmt;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskId {
    project_name: String,
    task_name: String,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub project_name: String,
    pub project_root: PathBuf,
    pub env: EnvMap,
    pub task_name: String,
    pub cmd: String,
    pub depends_on: Vec<String>,
    pub persistent: bool,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub ignore: Vec<String>,
    pub cache: bool,
    pub restart: RestartConfig,
}

pub type TaskMap = HashMap<TaskId, Task>;

impl TaskId {
    pub fn new(project_name: &str, task_name: &str) -> TaskId {
        TaskId {
            project_name: project_name.to_string(),
            task_name: task_name.to_string(),
        }
    }

    pub fn project_name(&self) -> &str {
        &self.project_name
    }

    pub fn task_name(&self) -> &str {
        &self.task_name
    }

    pub fn parse(s: &str) -> Option<TaskId> {
        let (project, task) = s.split_once(':')?;
        if project.is_empty() || task.is_empty() || task.contains(':') {
            return None;
        }
        Some(TaskId::new(project, task))
    }

    pub fn hash(&self) -> usize {
        let mut hasher = DefaultHasher::new();
        self.project_name.hash(&mut hasher);
        self.task_name.hash(&mut hasher);
        hasher.finish() as usize
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.project_name, self.task_name)
    }
}
