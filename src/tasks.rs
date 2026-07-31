use std::{collections::HashMap, fmt::Debug, os::unix::process::CommandExt, path::PathBuf};

use crate::config::{EnvMap, RestartConfig};

#[derive(Clone, PartialEq, Hash, Eq)]
pub struct TaskId {
    project_name: String,
    task_name: String,
}
#[derive(Clone)]
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

impl Task {
    pub fn execute(&self) {
        let cmd = &self.cmd;
        let mut child = std::process::Command::new("sh");
        for (arg, value) in &self.env {
            child.env(arg, value);
        }
        child.arg("-c").arg(cmd);
        let stdout = std::process::Stdio::inherit();
        let stderr = std::process::Stdio::inherit();
        child.stdout(stdout).stderr(stderr);
        let status = child.status().expect("Failed to execute command");
        if !status.success() {
            eprintln!("Command failed with status: {}", status);
        }
    }

    pub fn get_id(&self) -> TaskId {
        TaskId::new(&self.project_name, &self.task_name)
    }
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
    pub fn from_string(s: &str) -> Option<TaskId> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() == 2 {
            Some(TaskId::new(parts[0], parts[1]))
        } else {
            None
        }
    }
}

impl Debug for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.project_name, self.task_name)
    }
}

impl Debug for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Task")
            .field("\nproject_name", &self.project_name)
            .field("\ntask_name", &self.task_name)
            .field("\ncmd", &self.cmd)
            .field("\ndepends_on", &self.depends_on)
            .field("\npersistent", &self.persistent)
            .field("\ninputs", &self.inputs)
            .field("\noutputs", &self.outputs)
            .field("\nignore", &self.ignore)
            .field("\ncache", &self.cache)
            .finish()
    }
}
