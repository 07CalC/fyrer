use std::{
    collections::HashMap,
    fmt,
    hash::{DefaultHasher, Hash, Hasher},
    path::PathBuf,
};

use crate::config::RestartConfig;
use crate::env::EnvMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskId {
    project_name: String,
    task_name: String,
}

#[derive(Debug, Clone)]
pub struct Task {
    /// The name of the project this task belongs to.
    pub project_name: String,
    /// The root directory the task runs in.
    pub project_root: PathBuf,
    /// The resolved environment for the task.
    pub env: EnvMap,
    /// The task name within its project.
    pub task_name: String,
    /// The shell command to run.
    pub cmd: String,
    /// Names of tasks that must run first.
    pub depends_on: Vec<String>,
    /// Glob patterns of watched input files.
    pub inputs: Vec<String>,
    /// Glob patterns of produced output files.
    pub outputs: Vec<String>,
    /// Glob patterns of ignored files.
    pub ignore: Vec<String>,
    /// Whether the task may be skipped when its outputs are fresh.
    pub cache: bool,
    /// The restart policy for the task.
    pub restart: RestartConfig,
}

pub type TaskMap = HashMap<TaskId, Task>;

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

    /// Computes a deterministic hash used to assign log colors.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn hash(&self) -> usize {
        let mut hasher = DefaultHasher::new();
        Hash::hash(self, &mut hasher);
        hasher.finish() as usize
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.project_name, self.task_name)
    }
}

#[cfg(test)]
mod tests {
    use super::TaskId;

    #[test]
    fn parses_valid_ids() {
        let id = TaskId::parse("web:build").unwrap();
        assert_eq!(id.project_name(), "web");
        assert_eq!(id.task_name(), "build");
        assert_eq!(id.to_string(), "web:build");
    }

    #[test]
    fn rejects_malformed_ids() {
        assert!(TaskId::parse(":build").is_none());
        assert!(TaskId::parse("web:").is_none());
        assert!(TaskId::parse("web:build:extra").is_none());
        assert!(TaskId::parse("web").is_none());
    }
}
