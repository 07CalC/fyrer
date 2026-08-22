use std::{
    collections::{HashMap, HashSet},
    fs::File,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::Result;
use glob::Pattern;
use serde::Deserialize;

use crate::{
    cache::{CacheConfig, CacheProviderKind},
    error::{ConfigError, ValidationError},
    package::PackageConfig,
};

pub type EnvMap = HashMap<String, String>;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workspace {
    pub version: u32,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub env: EnvMap,
    pub packages: Vec<PackageConfig>,
    #[serde(skip)]
    pub workspace_root: PathBuf,
    // new optional fields for engine
    #[serde(default)]
    pub concurrency: Option<usize>,
}

// helper trait for $WORKSPACE resolution
trait ResolvePath {
    fn resolve_path(&self, ws: &Path) -> Option<PathBuf>;
}
impl ResolvePath for PathBuf {
    fn resolve_path(&self, ws: &Path) -> Option<PathBuf> {
        let mut comps = self.components();
        match comps.next() {
            Some(std::path::Component::Normal(first)) if first == "$WORKSPACE" => {
                Some(ws.join(comps.as_path()))
            }
            _ => None,
        }
    }
}

fn parse_env(content: &str) -> EnvMap {
    let mut map = EnvMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let k = k.trim();
            if !k.is_empty() {
                map.insert(k.to_string(), v.trim().to_string());
            }
        }
    }
    map
}

fn read_env_file(path: &Path) -> Result<EnvMap> {
    let content = std::fs::read_to_string(path)?;
    Ok(parse_env(&content))
}

fn merge_envs(
    global: &EnvMap,
    project_env: &EnvMap,
    task_env: &EnvMap,
    project_file: Option<&Path>,
    task_file: Option<&Path>,
) -> EnvMap {
    let pf = project_file
        .and_then(|p| read_env_file(p).ok())
        .unwrap_or_default();
    let tf = task_file
        .and_then(|p| read_env_file(p).ok())
        .unwrap_or_default();
    let mut m = global.clone();
    m.extend(pf);
    m.extend(project_env.clone());
    m.extend(tf);
    m.extend(task_env.clone());
    m
}

impl Workspace {
    pub fn new_from_path(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path.as_ref()).map_err(ConfigError::Io)?;
        let mut cfg: Self =
            serde_yaml::from_reader(file).map_err(ConfigError::Deserialization)?;
        cfg.workspace_root = path
            .as_ref()
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        Ok(cfg)
    }

    pub fn new_from_str(content: &str) -> Result<Self> {
        let mut cfg: Self =
            serde_yaml::from_str(content).map_err(ConfigError::Deserialization)?;
        cfg.workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Ok(cfg)
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

    fn validate_packages(&self) -> Result<()> {
        let mut names = HashSet::new();
        for pkg in &self.packages {
            if !names.insert(&pkg.name) {
                return Err(ConfigError::Validation(
                    ValidationError::DuplicatePackageName(pkg.name.clone()),
                )
                .into());
            }
            if pkg.root.as_os_str().is_empty() {
                return Err(ConfigError::Validation(ValidationError::EmptyProjectRoot {
                    project: pkg.name.clone(),
                })
                .into());
            }
            if pkg.root.is_absolute() {
                return Err(ConfigError::Validation(ValidationError::AbsoluteProjectRoot {
                    project: pkg.name.clone(),
                })
                .into());
            }
            let root = pkg
                .root
                .resolve_path(&self.workspace_root)
                .unwrap_or_else(|| pkg.root.clone());
            if !root.is_dir() {
                return Err(ConfigError::Validation(
                    ValidationError::ProjectRootDoesNotExist {
                        project: pkg.name.clone(),
                    },
                )
                .into());
            }
            if let Some(ef) = &pkg.env_file {
                if ef.is_absolute() {
                    return Err(ConfigError::Validation(ValidationError::AbsolutePath {
                        project: pkg.name.clone(),
                        task: "N/A".to_string(),
                        actor: "env_file".to_string(),
                    })
                    .into());
                }
                let ef = ef
                    .resolve_path(&self.workspace_root)
                    .unwrap_or_else(|| root.join(ef));
                if !ef.exists() {
                    return Err(ConfigError::Validation(ValidationError::EnvFileNotFound {
                        project: pkg.name.clone(),
                        task: "N/A".to_string(),
                        file: ef.to_string_lossy().to_string(),
                    })
                    .into());
                }
            }
        }
        Ok(())
    }

    fn validate_tasks(&self) -> Result<()> {
        for pkg in &self.packages {
            let root = pkg
                .root
                .resolve_path(&self.workspace_root)
                .unwrap_or_else(|| pkg.root.clone());
            let mut names = HashSet::new();
            for (tn, tc) in &pkg.tasks {
                if !names.insert(tn) {
                    return Err(ConfigError::Validation(ValidationError::DuplicateTaskName {
                        project: pkg.name.clone(),
                        task: tn.clone(),
                    })
                    .into());
                }
                if let Some(ef) = &tc.env_file {
                    if ef.is_absolute() {
                        return Err(ConfigError::Validation(ValidationError::AbsolutePath {
                            project: pkg.name.clone(),
                            task: tn.clone(),
                            actor: "env_file".to_string(),
                        })
                        .into());
                    }
                    let ef = ef
                        .resolve_path(&self.workspace_root)
                        .unwrap_or_else(|| root.join(ef));
                    if !ef.exists() {
                        return Err(ConfigError::Validation(ValidationError::EnvFileNotFound {
                            project: pkg.name.clone(),
                            task: tn.clone(),
                            file: ef.to_string_lossy().to_string(),
                        })
                        .into());
                    }
                }
                if tc.cmd.is_empty() {
                    return Err(ConfigError::Validation(ValidationError::EmptyCommand {
                        project: pkg.name.clone(),
                        task: tn.clone(),
                    })
                    .into());
                }
                if tc.timeout.is_some_and(|t| t.as_secs() == 0) {
                    return Err(ConfigError::Validation(ValidationError::InvalidTimeout {
                        project: pkg.name.clone(),
                        task: tn.clone(),
                    })
                    .into());
                }
                if let Some(cwd) = &tc.cwd {
                    if cwd.is_absolute() {
                        return Err(ConfigError::Validation(ValidationError::AbsolutePath {
                            project: pkg.name.clone(),
                            task: tn.clone(),
                            actor: "cwd".to_string(),
                        })
                        .into());
                    }
                    let cwd = cwd
                        .resolve_path(&self.workspace_root)
                        .unwrap_or_else(|| root.join(cwd));
                    if !cwd.exists() {
                        return Err(ConfigError::Validation(ValidationError::InvalidCwd {
                            project: pkg.name.clone(),
                            task: tn.clone(),
                        })
                        .into());
                    }
                }
                for pat in tc.ignore.iter().chain(tc.inputs.iter()).chain(tc.outputs.iter()) {
                    let actor = if tc.ignore.contains(pat) {
                        "ignore"
                    } else if tc.inputs.contains(pat) {
                        "inputs"
                    } else {
                        "outputs"
                    };
                    if Pattern::new(pat).is_err() {
                        return Err(ConfigError::Validation(
                            ValidationError::InvalidGlobPattern {
                                project: pkg.name.clone(),
                                task: tn.clone(),
                                actor: actor.to_string(),
                                pattern: pat.clone(),
                            },
                        )
                        .into());
                    }
                }
                if tc.cache && tc.persistent {
                    return Err(ConfigError::Validation(
                        ValidationError::CacheWithPersistentTask {
                            project: pkg.name.clone(),
                            task: tn.clone(),
                        },
                    )
                    .into());
                }
                if tc.cache && tc.watch {
                    return Err(ConfigError::Validation(ValidationError::CacheWithWatchTask {
                        project: pkg.name.clone(),
                        task: tn.clone(),
                    })
                    .into());
                }
            }
        }
        Ok(())
    }

    pub fn list_tasks(&self) {
        for pkg in &self.packages {
            println!("\n{}", pkg.name);
            for (tn, t) in &pkg.tasks {
                println!("  {}:", tn);
                println!("    command  {}", t.cmd);
                if let Some(cwd) = &t.cwd {
                    println!("    cwd      {}", cwd.display());
                }
                if !t.inputs.is_empty() {
                    println!("    inputs   {:?}", t.inputs);
                }
                if !t.outputs.is_empty() {
                    println!("    outputs  {:?}", t.outputs);
                }
                if !t.ignore.is_empty() {
                    println!("    ignore   {:?}", t.ignore);
                }
                if let Some(to) = &t.timeout {
                    println!("    timeout  {:?}", to);
                }
                if t.cache || t.persistent || t.watch {
                    let mut flags = Vec::new();
                    if t.cache {
                        flags.push("cache");
                    }
                    if t.persistent {
                        flags.push("persistent");
                    }
                    if t.watch {
                        flags.push("watch");
                    }
                    println!("    flags    {}", flags.join(", "));
                }
            }
        }
    }

    /// Build a TaskRegistry (specs) from workspace.
    pub fn task_registry(&self) -> fyrer_core::spec::TaskRegistry {
        use fyrer_core::{TaskId, spec::TaskSpec};
        let mut map = HashMap::new();
        for pkg in &self.packages {
            let package_root = pkg
                .root
                .resolve_path(&self.workspace_root)
                .unwrap_or_else(|| pkg.root.clone());
            let pkg_env_file = pkg.env_file.as_ref().map(|f| {
                f.resolve_path(&self.workspace_root)
                    .unwrap_or_else(|| package_root.join(f))
            });
            for (tn, t) in &pkg.tasks {
                let id = TaskId::new(&pkg.name, tn);
                let cwd = t.cwd.as_ref().map_or_else(
                    || package_root.clone(),
                    |cwd| {
                        cwd.resolve_path(&self.workspace_root)
                            .unwrap_or_else(|| package_root.join(cwd))
                    },
                );
                let task_env_file = t.env_file.as_ref().map(|f| {
                    f.resolve_path(&self.workspace_root)
                        .unwrap_or_else(|| package_root.join(f))
                });
                let env = merge_envs(
                    &self.env,
                    &pkg.env,
                    &t.env,
                    pkg_env_file.as_deref(),
                    task_env_file.as_deref(),
                );
                let spec = Arc::new(TaskSpec::new(
                    id.clone(),
                    env,
                    t.cache,
                    t.watch,
                    t.persistent,
                    t.timeout,
                    cwd,
                    t.cmd.clone(),
                    t.depends_on
                        .iter()
                        .map(|dep| {
                            if dep.contains(':') {
                                let p: Vec<&str> = dep.split(':').collect();
                                TaskId::new(p[0], p[1])
                            } else {
                                TaskId::new(&pkg.name, dep)
                            }
                        })
                        .collect(),
                    t.inputs.clone(),
                    t.outputs.clone(),
                    t.ignore.clone(),
                ));
                map.insert(id, spec);
            }
        }
        fyrer_core::spec::TaskRegistry::new(map)
    }

    pub fn task_graph(&self) -> Result<fyrer_core::TaskGraph, fyrer_core::graph::GraphError> {
        let registry = self.task_registry();
        let specs: Vec<_> = registry
            .iter()
            .map(|(id, s)| (id.clone(), s.depends_on.clone()))
            .collect();
        let g = fyrer_core::TaskGraph::from_specs(specs)?;
        g.validate()?;
        Ok(g)
    }
}
