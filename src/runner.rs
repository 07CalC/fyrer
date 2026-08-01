//! High-level orchestration: loading config, resolving tasks, and running them.

use std::{collections::BTreeMap, path::Path};

use crate::{
    config::FyrerConfig,
    error::{FyrerError, FyrerResult, GraphError},
    executor, global,
    graph::TaskGraph,
    logger::Logger,
    tasks::{TaskId, TaskMap},
    watcher,
};

pub struct Runner {
    task_map: TaskMap,
    task_graph: TaskGraph,
}

impl Runner {
    pub fn load(path: impl AsRef<Path>) -> FyrerResult<Self> {
        let config = FyrerConfig::new_from_path(path)?;
        Self::from_config(&config)
    }

    pub fn from_config(config: &FyrerConfig) -> FyrerResult<Self> {
        let task_map = config.create_task_map()?;
        let task_graph = TaskGraph::new(&task_map)?;
        task_graph.validate()?;
        Ok(Self {
            task_map,
            task_graph,
        })
    }

    /// Prints every project and its tasks, grouped and sorted by project.
    ///
    /// # Errors
    ///
    /// This function is infallible; it always returns `Ok`.
    pub fn list(&self) -> FyrerResult<()> {
        let mut projects: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for id in self.task_map.keys() {
            projects
                .entry(id.project_name())
                .or_default()
                .push(id.task_name());
        }
        for (project, mut tasks) in projects {
            tasks.sort_unstable();
            println!("{project}");
            for task in tasks {
                println!("  {task}");
            }
        }
        Ok(())
    }

    /// Resolves a CLI task specifier into one or more task ids.
    ///
    /// A `project:task` specifier selects a single task; a bare `task` name
    /// selects every task with that name; `None` selects every task.
    ///
    /// # Errors
    ///
    /// Returns an error if the specifier is malformed or does not match any
    /// task.
    pub fn resolve(&self, spec: Option<&str>) -> FyrerResult<Vec<TaskId>> {
        match spec {
            None => {
                let mut all: Vec<TaskId> = self.task_map.keys().cloned().collect();
                all.sort_by_key(ToString::to_string);
                Ok(all)
            }
            Some(spec) if spec.contains(':') => {
                let id = TaskId::parse(spec).ok_or_else(|| {
                    FyrerError::Graph(GraphError::InvalidTaskId {
                        dependency: spec.to_string(),
                        task: spec.to_string(),
                    })
                })?;
                if !self.task_map.contains_key(&id) {
                    return Err(FyrerError::Graph(GraphError::TaskNotFound(
                        spec.to_string(),
                    )));
                }
                Ok(vec![id])
            }
            Some(spec) => {
                let mut matches: Vec<TaskId> = self
                    .task_map
                    .keys()
                    .filter(|id| id.task_name() == spec)
                    .cloned()
                    .collect();
                matches.sort_by_key(ToString::to_string);
                if matches.is_empty() {
                    return Err(FyrerError::Graph(GraphError::TaskNotFound(
                        spec.to_string(),
                    )));
                }
                Ok(matches)
            }
        }
    }

    /// Prints the topological execution plan for the given tasks.
    ///
    /// # Errors
    ///
    /// Returns an error if any of the given tasks is not in the task graph.
    pub fn plan(&self, tasks: &[TaskId]) -> FyrerResult<()> {
        let levels = self.task_graph.get_orders(tasks)?;
        for (index, level) in levels.iter().enumerate() {
            let names: Vec<String> = level.iter().map(ToString::to_string).collect();
            println!("step {}: {}", index + 1, names.join(", "));
        }
        Ok(())
    }

    /// Executes the given tasks and watches the long-running ones.
    ///
    /// # Errors
    ///
    /// Returns an error if the global state cannot be initialized, a task
    /// fails to execute, or a file watcher cannot be started.
    pub async fn run(&self, tasks: &[TaskId]) -> FyrerResult<()> {
        let mut logger = Logger::new(self.task_map.len());
        let log_sender = logger.sender();
        tokio::spawn(async move {
            logger.start().await;
        });
        global::init(self.task_graph.clone(), self.task_map.clone(), log_sender)?;
        tokio::spawn(global::await_shutdown_signal());
        let running = executor::execute_tasks(tasks).await?;
        tokio::select! {
            result = watcher::watch_tasks(running) => result,
            () = global::shutdown_notified() => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Runner;
    use crate::{config::FyrerConfig, tasks::TaskId};

    const YAML: &str = r"
version: 1
projects:
  - name: web
    root: ./
    env_path: .env
    tasks:
      build: { cmd: echo build }
      test: { cmd: echo test }
  - name: api
    root: ./
    env_path: .env
    tasks:
      build: { cmd: echo build }
";

    fn runner(yaml: &str) -> Runner {
        Runner::from_config(&FyrerConfig::new_from_str(yaml).unwrap()).unwrap()
    }

    #[test]
    fn resolve_all() {
        let runner = runner(YAML);
        assert_eq!(runner.resolve(None).unwrap().len(), 3);
    }

    #[test]
    fn resolve_specific() {
        let runner = runner(YAML);
        let tasks = runner.resolve(Some("web:build")).unwrap();
        assert_eq!(tasks, vec![TaskId::new("web", "build")]);
    }

    #[test]
    fn resolve_by_name() {
        let runner = runner(YAML);
        let tasks = runner.resolve(Some("build")).unwrap();
        let names: Vec<String> = tasks.iter().map(ToString::to_string).collect();
        assert_eq!(names, vec!["api:build", "web:build"]);
    }

    #[test]
    fn resolve_missing() {
        let runner = runner(YAML);
        assert!(runner.resolve(Some("nope")).is_err());
        assert!(runner.resolve(Some("web:nope")).is_err());
    }
}
