//! Workspace config: schema, loading, validation and lowering to the
//! [`TaskRegistry`] / [`TaskGraph`] used by the engine.

use std::{
    collections::HashMap,
    fs::File,
    path::{Path, PathBuf},
};

use anyhow::Result;
use glob::Pattern;
use serde::Deserialize;

pub use crate::env::EnvMap;
use crate::{
    cache::CacheConfig,
    env::merge_envs,
    error::{ConfigError, ValidationError},
    package::PackageConfig,
    paths::ResolvePath,
    task::TaskConfig,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workspace {
    pub version: u32,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub env: EnvMap,
    pub packages: Vec<PackageConfig>,
    /// Root directory of the workspace, derived from the config file location.
    /// `$WORKSPACE`-prefixed paths resolve against this.
    #[serde(skip)]
    pub workspace_root: PathBuf,
    /// Max concurrently running tasks. Defaults to available parallelism.
    #[serde(default)]
    pub concurrency: Option<usize>,
}

impl Workspace {
    pub fn new_from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let file = File::open(path).map_err(ConfigError::Io)?;
        let mut workspace: Self =
            serde_yaml::from_reader(file).map_err(ConfigError::Deserialization)?;
        workspace.workspace_root = path
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        Ok(workspace)
    }

    #[allow(dead_code)]
    pub fn new_from_str(content: &str) -> Result<Self> {
        let mut workspace: Self =
            serde_yaml::from_str(content).map_err(ConfigError::Deserialization)?;
        workspace.workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Ok(workspace)
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err(ConfigError::Validation(ValidationError::UnsupportedVersion(
                self.version,
            ))
            .into());
        }
        self.validate_packages()?;
        self.validate_tasks()?;
        Ok(())
    }

    /// Resolve a possibly `$WORKSPACE`-prefixed path, falling back to the raw
    /// path joined under `base`.
    fn resolve(&self, base: &Path, path: &Path) -> PathBuf {
        path.resolve_path(&self.workspace_root)
            .unwrap_or_else(|| base.join(path))
    }

    fn validate_packages(&self) -> Result<()> {
        let mut names = std::collections::HashSet::new();
        for package in &self.packages {
            if !names.insert(&package.name) {
                return Err(ValidationError::DuplicatePackageName(package.name.clone()).into_config_error());
            }
            if package.root.as_os_str().is_empty() {
                return Err(ValidationError::EmptyProjectRoot {
                    project: package.name.clone(),
                }
                .into_config_error());
            }
            if package.root.is_absolute() {
                return Err(ValidationError::AbsoluteProjectRoot {
                    project: package.name.clone(),
                }
                .into_config_error());
            }
            let root = self.resolve(&self.workspace_root, &package.root);
            if !root.is_dir() {
                return Err(ValidationError::ProjectRootDoesNotExist {
                    project: package.name.clone(),
                }
                .into_config_error());
            }
            if let Some(env_file) = &package.env_file {
                self.validate_env_file(&package.name, "N/A", &root, env_file)?;
            }
        }
        Ok(())
    }

    fn validate_tasks(&self) -> Result<()> {
        for package in &self.packages {
            let root = self.resolve(&self.workspace_root, &package.root);
            let mut task_names = std::collections::HashSet::new();
            for (task_name, task) in &package.tasks {
                if !task_names.insert(task_name) {
                    return Err(ValidationError::DuplicateTaskName {
                        project: package.name.clone(),
                        task: task_name.clone(),
                    }
                    .into_config_error());
                }
                self.validate_task(package.name.as_str(), task_name, task, &root)?;
            }
        }
        Ok(())
    }

    fn validate_task(
        &self,
        project: &str,
        task_name: &str,
        task: &TaskConfig,
        package_root: &Path,
    ) -> Result<()> {
        let err = |e: ValidationError| -> anyhow::Error { e.into_config_error() };

        if task.cmd.is_empty() {
            return Err(err(ValidationError::EmptyCommand {
                project: project.to_string(),
                task: task_name.to_string(),
            }));
        }
        if task.timeout.is_some_and(|t| t.as_secs() == 0) {
            return Err(err(ValidationError::InvalidTimeout {
                project: project.to_string(),
                task: task_name.to_string(),
            }));
        }

        if let Some(env_file) = &task.env_file {
            self.validate_env_file(project, task_name, package_root, env_file)?;
        }

        if let Some(cwd) = &task.cwd {
            if cwd.is_absolute() {
                return Err(err(ValidationError::AbsolutePath {
                    project: project.to_string(),
                    task: task_name.to_string(),
                    actor: "cwd".to_string(),
                }));
            }
            let resolved = cwd
                .resolve_path(&self.workspace_root)
                .unwrap_or_else(|| package_root.join(cwd));
            if !resolved.exists() {
                return Err(err(ValidationError::InvalidCwd {
                    project: project.to_string(),
                    task: task_name.to_string(),
                }));
            }
        }

        // Glob patterns must be relative and well-formed.
        for (actor, patterns) in [
            ("ignore", &task.ignore),
            ("inputs", &task.inputs),
            ("outputs", &task.outputs),
        ] {
            for pattern in patterns {
                if Pattern::new(pattern).is_err() {
                    return Err(err(ValidationError::InvalidGlobPattern {
                        project: project.to_string(),
                        task: task_name.to_string(),
                        actor: actor.to_string(),
                        pattern: pattern.clone(),
                    }));
                }
            }
        }

        // Mutually exclusive execution modes.
        if task.cache && task.persistent {
            return Err(err(ValidationError::CacheWithPersistentTask {
                project: project.to_string(),
                task: task_name.to_string(),
            }));
        }
        if task.cache && task.watch {
            return Err(err(ValidationError::CacheWithWatchTask {
                project: project.to_string(),
                task: task_name.to_string(),
            }));
        }
        Ok(())
    }

    fn validate_env_file(
        &self,
        project: &str,
        task_name: &str,
        package_root: &Path,
        env_file: &Path,
    ) -> Result<()> {
        if env_file.is_absolute() {
            return Err(ValidationError::AbsolutePath {
                project: project.to_string(),
                task: task_name.to_string(),
                actor: "env_file".to_string(),
            }
            .into_config_error());
        }
        let resolved = self.resolve(package_root, env_file);
        if !resolved.exists() {
            return Err(ValidationError::EnvFileNotFound {
                project: project.to_string(),
                task: task_name.to_string(),
                file: resolved.to_string_lossy().to_string(),
            }
            .into_config_error());
        }
        Ok(())
    }

    pub fn list_tasks(&self) {
        for package in &self.packages {
            println!("\n{}", package.name);
            for (task_name, task) in &package.tasks {
                println!("  {task_name}:");
                println!("    command  {}", task.cmd);
                if let Some(cwd) = &task.cwd {
                    println!("    cwd      {}", cwd.display());
                }
                if !task.inputs.is_empty() {
                    println!("    inputs   {:?}", task.inputs);
                }
                if !task.outputs.is_empty() {
                    println!("    outputs  {:?}", task.outputs);
                }
                if !task.ignore.is_empty() {
                    println!("    ignore   {:?}", task.ignore);
                }
                if let Some(timeout) = &task.timeout {
                    println!("    timeout  {timeout:?}");
                }
                if task.cache || task.persistent || task.watch {
                    let flags: Vec<&str> = [
                        ("cache", task.cache),
                        ("persistent", task.persistent),
                        ("watch", task.watch),
                    ]
                    .iter()
                    .filter(|(_, on)| *on)
                    .map(|(name, _)| *name)
                    .collect();
                    println!("    flags    {}", flags.join(", "));
                }
            }
        }
    }

    /// Lower the workspace into an immutable engine-facing registry of fully
    /// resolved task specs (absolute cwds, merged environments, typed deps).
    pub fn task_registry(&self) -> fyrer_core::spec::TaskRegistry {
        use fyrer_core::{TaskId, spec::TaskSpec};
        let mut map = HashMap::new();
        for package in &self.packages {
            let package_root = self.resolve(&self.workspace_root, &package.root);
            let package_env_file = package
                .env_file
                .as_ref()
                .map(|f| self.resolve(&package_root, f));
            for (task_name, task) in &package.tasks {
                let id = TaskId::new(&package.name, task_name);
                let cwd = match &task.cwd {
                    Some(cwd) => self.resolve(&package_root, cwd),
                    None => package_root.clone(),
                };
                let task_env_file = task
                    .env_file
                    .as_ref()
                    .map(|f| self.resolve(&package_root, f));
                let env = merge_envs(
                    &self.env,
                    &package.env,
                    &task.env,
                    package_env_file.as_deref(),
                    task_env_file.as_deref(),
                );
                map.insert(
                    id.clone(),
                    std::sync::Arc::new(TaskSpec::new(
                        id.clone(),
                        env,
                        task.cache,
                        task.watch,
                        task.persistent,
                        task.timeout,
                        cwd,
                        task.cmd.clone(),
                        task.depends_on.iter().map(|dep| self.resolve_dep(&package.name, dep)).collect(),
                        task.inputs.clone(),
                        task.outputs.clone(),
                        task.ignore.clone(),
                    )),
                );
            }
        }
        fyrer_core::spec::TaskRegistry::new(map)
    }

    fn resolve_dep(&self, package: &str, dep: &str) -> fyrer_core::TaskId {
        match dep.split_once(':') {
            Some((other_package, other_task)) => {
                fyrer_core::TaskId::new(other_package, other_task)
            }
            None => fyrer_core::TaskId::new(package, dep),
        }
    }

    pub fn task_graph(&self) -> Result<fyrer_core::TaskGraph, fyrer_core::graph::GraphError> {
        let graph = fyrer_core::TaskGraph::from_specs(
            self.task_registry()
                .iter()
                .map(|(id, spec)| (id.clone(), spec.depends_on.clone())),
        )?;
        graph.validate()?;
        Ok(graph)
    }
}
