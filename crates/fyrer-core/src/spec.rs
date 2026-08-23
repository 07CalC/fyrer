use std::{collections::HashMap, path::PathBuf, time::Duration};

use crate::id::TaskId;

/// Immutable task definition — pure data, no behavior.
/// Mirrors previous `Task` struct without spawn logic.
#[derive(Debug, Clone)]
pub struct TaskSpec {
    pub id: TaskId,
    pub env: HashMap<String, String>,
    pub cacheable: bool,
    pub watch: bool,
    pub persistent: bool,
    pub timeout: Option<Duration>,
    pub cwd: PathBuf,
    pub cmd: String,
    pub depends_on: Vec<TaskId>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub ignore: Vec<String>,
}

impl TaskSpec {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: TaskId,
        env: HashMap<String, String>,
        cacheable: bool,
        watch: bool,
        persistent: bool,
        timeout: Option<Duration>,
        cwd: PathBuf,
        cmd: String,
        depends_on: Vec<TaskId>,
        inputs: Vec<String>,
        outputs: Vec<String>,
        ignore: Vec<String>,
    ) -> Self {
        Self {
            id,
            env,
            cacheable,
            watch,
            persistent,
            timeout,
            cwd,
            cmd,
            depends_on,
            inputs,
            outputs,
            ignore,
        }
    }
}

/// Registry of all specs: TaskId -> spec
#[derive(Debug, Clone, Default)]
pub struct TaskRegistry {
    inner: HashMap<TaskId, std::sync::Arc<TaskSpec>>,
}

impl TaskRegistry {
    pub fn new(map: HashMap<TaskId, std::sync::Arc<TaskSpec>>) -> Self {
        Self { inner: map }
    }
    pub fn get(&self, id: &TaskId) -> Option<std::sync::Arc<TaskSpec>> {
        self.inner.get(id).cloned()
    }
    pub fn contains(&self, id: &TaskId) -> bool {
        self.inner.contains_key(id)
    }
    pub fn ids(&self) -> Vec<TaskId> {
        self.inner.keys().cloned().collect()
    }
    pub fn len(&self) -> usize {
        self.inner.len()
    }
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = (&TaskId, &std::sync::Arc<TaskSpec>)> {
        self.inner.iter()
    }
    pub fn into_inner(self) -> HashMap<TaskId, std::sync::Arc<TaskSpec>> {
        self.inner
    }
}
