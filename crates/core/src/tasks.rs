use std::{collections::HashMap, path::PathBuf};

use crate::config::{EnvMap, RestartConfig};

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
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

    pub fn to_string(&self) -> String {
        format!("{}:{}", self.project_name, self.task_name)
    }
}
