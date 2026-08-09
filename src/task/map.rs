use std::{collections::HashMap, path::PathBuf, sync::Arc};

use crate::{TaskId, config::FyrerConfig, env::merge_envs, task::Task};

#[derive(Debug, Clone)]
pub struct TaskMap {
    pub tasks: HashMap<TaskId, Arc<Task>>,
}

impl From<FyrerConfig> for TaskMap {
    fn from(value: FyrerConfig) -> Self {
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
                        .map(|dep| TaskId::new(&package.name, dep))
                        .collect(),
                    inputs: task.inputs.clone(),
                    outputs: task.outputs.clone(),
                    ignore: task.ignore.clone(),
                });
                tasks.insert(task_id, task);
            }
        }
        Self { tasks }
    }
}
