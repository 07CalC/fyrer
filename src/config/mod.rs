use std::fs::File;
use std::{collections::HashSet, path::Path};

use anyhow::Result;
use glob::Pattern;
use serde::Deserialize;

use crate::{config::package::PackageConfig, utils::env::EnvMap};

mod cache;
mod error;
mod package;
mod task;
pub(crate) use cache::CacheConfig;
pub(crate) use cache::CacheProviderKind;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FyrerConfig {
    pub version: u32,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub env: EnvMap,
    pub packages: Vec<PackageConfig>,
}

impl FyrerConfig {
    pub fn new_from_path(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path.as_ref()).map_err(|e| error::ConfigError::Io(e))?;
        let config: Self =
            serde_yaml::from_reader(file).map_err(|e| error::ConfigError::Deserialization(e))?;
        Ok(config)
    }

    #[allow(dead_code)]
    fn new_from_str(content: &str) -> Result<Self> {
        let config: Self =
            serde_yaml::from_str(content).map_err(|e| error::ConfigError::Deserialization(e))?;
        Ok(config)
    }

    /// general validation rules:
    /// 1. version must be 1 (for now)
    /// 2. paths should be relative to keep things simple and portable
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err(error::ConfigError::Validation(
                error::ValidationError::UnsupportedVersion(self.version),
            )
            .into());
        }
        self.validate_packages()?;
        self.validate_tasks()?;
        Ok(())
    }

    /// validation rules:
    /// 1. package names must be unique
    /// 2. package root must not be empty
    /// 3. package root must exist
    /// 4. package root must be relative
    /// 5. env_file must be relative and exist if specified
    fn validate_packages(&self) -> Result<()> {
        let mut package_names = HashSet::new();
        for package in &self.packages {
            if !package_names.insert(&package.name) {
                return Err(error::ConfigError::Validation(
                    error::ValidationError::DuplicatePackageName(package.name.clone()),
                )
                .into());
            }
            if package.root.as_os_str().is_empty() {
                return Err(error::ConfigError::Validation(
                    error::ValidationError::EmptyProjectRoot {
                        project: package.name.clone(),
                    },
                )
                .into());
            }
            if !package.root.is_dir() {
                return Err(error::ConfigError::Validation(
                    error::ValidationError::ProjectRootDoesNotExist {
                        project: package.name.clone(),
                    },
                )
                .into());
            }
            if package.root.is_absolute() {
                return Err(error::ConfigError::Validation(
                    error::ValidationError::AbsoluteProjectRoot {
                        project: package.name.clone(),
                    },
                )
                .into());
            }
            if let Some(env_file) = &package.env_file {
                if package.root.join(env_file).is_absolute() {
                    return Err(error::ConfigError::Validation(
                        error::ValidationError::AbsolutePath {
                            project: package.name.clone(),
                            task: "N/A".to_string(),
                            actor: "env_file".to_string(),
                        },
                    )
                    .into());
                }
                if !package.root.join(env_file).exists() {
                    return Err(error::ConfigError::Validation(
                        error::ValidationError::EnvFileNotFound {
                            project: package.name.clone(),
                            task: "N/A".to_string(),
                            file: env_file.to_string_lossy().to_string(),
                        },
                    )
                    .into());
                }
            }
        }
        Ok(())
    }

    /// validation rules:
    /// 1. task names must be unique within a package
    /// 2. task command must not be empty
    /// 3. task timeout must be greater than zero if specified
    /// 4. task cwd must be relative and exist within the package root if specified
    /// 5. task ignore, inputs, and outputs must be relative and valid glob patterns
    /// 6. task cannot be both cache and persistent
    /// 7. task cannot be both cache and watch
    /// 8. task env_file must be relative and exist if specified
    fn validate_tasks(&self) -> Result<()> {
        for package in &self.packages {
            let mut task_names = HashSet::new();
            for (task_name, task_config) in &package.tasks {
                if !task_names.insert(task_name) {
                    return Err(error::ConfigError::Validation(
                        error::ValidationError::DuplicateTaskName {
                            project: package.name.clone(),
                            task: task_name.clone(),
                        },
                    )
                    .into());
                }
                if let Some(env_file) = &task_config.env_file {
                    if package.root.join(env_file).is_absolute() {
                        return Err(error::ConfigError::Validation(
                            error::ValidationError::AbsolutePath {
                                project: package.name.clone(),
                                task: task_name.clone(),
                                actor: "env_file".to_string(),
                            },
                        )
                        .into());
                    }
                    if !package.root.join(env_file).exists() {
                        return Err(error::ConfigError::Validation(
                            error::ValidationError::EnvFileNotFound {
                                project: package.name.clone(),
                                task: task_name.clone(),
                                file: env_file.to_string_lossy().to_string(),
                            },
                        )
                        .into());
                    }
                }
                if task_config.cmd.is_empty() {
                    return Err(error::ConfigError::Validation(
                        error::ValidationError::EmptyCommand {
                            project: package.name.clone(),
                            task: task_name.clone(),
                        },
                    )
                    .into());
                }
                if task_config.timeout.is_some_and(|t| t.as_secs() == 0) {
                    return Err(error::ConfigError::Validation(
                        error::ValidationError::InvalidTimeout {
                            project: package.name.clone(),
                            task: task_name.clone(),
                        },
                    )
                    .into());
                }
                if let Some(cwd) = &task_config.cwd {
                    if package.root.join(cwd).is_absolute() {
                        return Err(error::ConfigError::Validation(
                            error::ValidationError::AbsolutePath {
                                project: package.name.clone(),
                                task: task_name.clone(),
                                actor: "cwd".to_string(),
                            },
                        )
                        .into());
                    }
                    if !package.root.join(cwd).exists() {
                        return Err(error::ConfigError::Validation(
                            error::ValidationError::InvalidCwd {
                                project: package.name.clone(),
                                task: task_name.clone(),
                            },
                        )
                        .into());
                    }
                }
                for ignore in &task_config.ignore {
                    if package.root.join(ignore).is_absolute() {
                        return Err(error::ConfigError::Validation(
                            error::ValidationError::AbsolutePath {
                                project: package.name.clone(),
                                task: task_name.clone(),
                                actor: "ignore".to_string(),
                            },
                        )
                        .into());
                    }
                    if !Pattern::new(ignore).is_ok() {
                        return Err(error::ConfigError::Validation(
                            error::ValidationError::InvalidGlobPattern {
                                project: package.name.clone(),
                                task: task_name.clone(),
                                actor: "ignore".to_string(),
                                pattern: ignore.clone(),
                            },
                        )
                        .into());
                    }
                }
                for input in &task_config.inputs {
                    if package.root.join(input).is_absolute() {
                        return Err(error::ConfigError::Validation(
                            error::ValidationError::AbsolutePath {
                                project: package.name.clone(),
                                task: task_name.clone(),
                                actor: "inputs".to_string(),
                            },
                        )
                        .into());
                    }
                    if !Pattern::new(input).is_ok() {
                        return Err(error::ConfigError::Validation(
                            error::ValidationError::InvalidGlobPattern {
                                project: package.name.clone(),
                                task: task_name.clone(),
                                actor: "inputs".to_string(),
                                pattern: input.clone(),
                            },
                        )
                        .into());
                    }
                }
                for output in &task_config.outputs {
                    if package.root.join(output).is_absolute() {
                        return Err(error::ConfigError::Validation(
                            error::ValidationError::AbsolutePath {
                                project: package.name.clone(),
                                task: task_name.clone(),
                                actor: "outputs".to_string(),
                            },
                        )
                        .into());
                    }
                    if !Pattern::new(output).is_ok() {
                        return Err(error::ConfigError::Validation(
                            error::ValidationError::InvalidGlobPattern {
                                project: package.name.clone(),
                                task: task_name.clone(),
                                actor: "outputs".to_string(),
                                pattern: output.clone(),
                            },
                        )
                        .into());
                    }
                }
                if task_config.cache && task_config.persistent {
                    return Err(error::ConfigError::Validation(
                        error::ValidationError::CacheWithPersistentTask {
                            project: package.name.clone(),
                            task: task_name.clone(),
                        },
                    )
                    .into());
                }
                if task_config.cache && task_config.watch {
                    return Err(error::ConfigError::Validation(
                        error::ValidationError::CacheWithWatchTask {
                            project: package.name.clone(),
                            task: task_name.clone(),
                        },
                    )
                    .into());
                }
            }
        }
        Ok(())
    }

    pub fn list_tasks(&self) {
        for package in &self.packages {
            println!("\n{}", package.name);
            for (task_name, task) in &package.tasks {
                println!("  {}:", task_name);
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
                    println!("    timeout  {:?}", timeout);
                }
                if task.cache || task.persistent || task.watch {
                    let mut flags = Vec::new();
                    if task.cache {
                        flags.push("cache");
                    }
                    if task.persistent {
                        flags.push("persistent");
                    }
                    if task.watch {
                        flags.push("watch");
                    }
                    println!("    flags    {}", flags.join(", "));
                }
            }
        }
    }
}
