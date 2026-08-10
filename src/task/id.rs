use std::{fmt::Display, sync::Arc};

/// keeping the project name and task name as Arc<String> to avoid cloning the strings when
/// passing around TaskId, since they will be cloned frequently throughout
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct TaskId {
    project_name: Arc<String>,
    task_name: Arc<String>,
}

impl TaskId {
    pub fn new(project_name: &str, task_name: &str) -> Self {
        Self {
            project_name: Arc::new(project_name.to_string()),
            task_name: Arc::new(task_name.to_string()),
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return None;
        }
        Some(Self::new(parts[0], parts[1]))
    }

    pub fn project_name(&self) -> &str {
        &self.project_name
    }

    pub fn task_name(&self) -> &str {
        &self.task_name
    }
}

impl Clone for TaskId {
    fn clone(&self) -> Self {
        Self {
            project_name: Arc::clone(&self.project_name),
            task_name: Arc::clone(&self.task_name),
        }
    }
}

impl From<&str> for TaskId {
    fn from(s: &str) -> Self {
        Self::from_str(s).expect("Invalid TaskId format")
    }
}

impl Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.project_name, self.task_name)
    }
}
