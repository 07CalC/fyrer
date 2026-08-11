use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::Result;

use crate::{
    config::FyrerConfig,
    env::merge_envs,
    task::{Task, TaskId},
};

/// task map will be immutable after creation, so we can use Arc to share tasks between threads
#[derive(Debug, Clone)]
pub struct TaskMap {
    pub tasks: Arc<HashMap<TaskId, Arc<Task>>>,
}

impl TaskMap {
    pub fn new(config: &FyrerConfig) -> Self {
        config.into()
    }
    pub fn get(&self, task_id: &TaskId) -> Option<Arc<Task>> {
        self.tasks.get(task_id).cloned()
    }
    pub fn get_tasks(&self, spec: Option<&str>) -> Result<Vec<TaskId>> {
        match spec {
            Some(spec) if spec.contains(':') => {
                let parts: Vec<&str> = spec.split(':').collect();
                if parts.len() != 2 {
                    return Err(anyhow::anyhow!(
                        "Invalid task specifier: {}. Expected format:\n1.package:task\n2.task\n3.empty for all tasks",
                        spec
                    ));
                }
                let package_name = parts[0];
                let task_name = parts[1];
                let task_id = TaskId::new(package_name, task_name);
                if self.tasks.contains_key(&task_id) {
                    Ok(vec![task_id])
                } else {
                    return Err(anyhow::anyhow!("Task {} not found", task_id));
                }
            }
            Some(spec) => {
                let task_ids: Vec<TaskId> = self
                    .tasks
                    .keys()
                    .filter(|task_id| task_id.task_name() == spec)
                    .cloned()
                    .collect();
                if task_ids.is_empty() {
                    return Err(anyhow::anyhow!("Task {} not found", spec));
                }
                Ok(task_ids)
            }
            None => Ok(self.tasks.keys().cloned().collect()),
        }
    }
}

impl From<&FyrerConfig> for TaskMap {
    fn from(value: &FyrerConfig) -> Self {
        let mut tasks = HashMap::new();
        for package in &value.packages {
            let package_env_file = package
                .env_file
                .as_ref()
                .map(|file| package.root.join(file));
            for (task_name, task) in &package.tasks {
                let task_id = TaskId::new(&package.name, task_name);
                let cwd = package
                    .root
                    .join(&task.cwd.as_ref().unwrap_or(&PathBuf::from(".")));
                let task_env_file = task.env_file.as_ref().map(|file| package.root.join(file));
                let env = merge_envs(
                    &value.env,
                    &package.env,
                    &task.env,
                    package_env_file.as_deref(),
                    task_env_file.as_deref(),
                );
                let task = Arc::new(Task {
                    id: task_id.clone(),
                    env,
                    cache: task.cache,
                    watch: task.watch,
                    persistent: task.persistent,
                    timeout: task.timeout,
                    cwd,
                    cmd: task.cmd.clone(),
                    depends_on: task
                        .depends_on
                        .iter()
                        .map(|dep| {
                            if dep.contains(':') {
                                let parts: Vec<&str> = dep.split(':').collect();
                                TaskId::new(parts[0], parts[1])
                            } else {
                                TaskId::new(&package.name, dep)
                            }
                        })
                        .collect(),
                    inputs: task.inputs.clone(),
                    outputs: task.outputs.clone(),
                    ignore: task.ignore.clone(),
                });
                tasks.insert(task_id, task);
            }
        }
        Self {
            tasks: Arc::new(tasks),
        }
    }
}
