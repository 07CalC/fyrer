use crate::env::{EnvMap, get_task_env_var, merge};
use crate::error::{FyrerError, FyrerResult, config::ConfigError};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use crate::tasks::{Task, TaskId, TaskMap};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FyrerConfig {
    pub version: u32,
    #[serde(default = "default_env_map")]
    pub env: EnvMap,
    pub projects: Vec<ProjectConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    pub name: String,
    pub root: PathBuf,
    #[serde(default = "default_env_map")]
    pub env: EnvMap,
    #[serde(default = "default_env_path")]
    pub env_path: String,
    #[serde(default = "default_tasks")]
    pub tasks: HashMap<String, TaskConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskConfig {
    #[serde(default = "default_cmd")]
    pub cmd: String,
    #[serde(default = "default_vec_string")]
    pub depends_on: Vec<String>,
    #[serde(default = "default_vec_string")]
    pub inputs: Vec<String>,
    #[serde(default = "default_vec_string")]
    pub outputs: Vec<String>,
    #[serde(default = "default_vec_string")]
    pub ignore: Vec<String>,
    #[serde(default = "default_bool")]
    pub cache: bool,
    #[serde(default = "default_restart")]
    pub restart: RestartConfig,
    #[serde(default = "default_env_map")]
    pub env: EnvMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartConfig {
    pub strategy: RestartStrategy,
    pub delay: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RestartStrategy {
    FileChange,
    OnFailure,
    Never,
}

impl FyrerConfig {
    pub fn new_from_path(path: &str) -> FyrerResult<FyrerConfig> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            FyrerError::Config(ConfigError::ReadFile {
                path: path.to_string(),
                source: e,
            })
        })?;

        let config: FyrerConfig = serde_yaml::from_str(content.as_str())
            .map_err(|e| FyrerError::Config(ConfigError::ParseYaml(e)))?;
        config.validate()?;
        Ok(config)
    }

    pub fn new_from_str(content: &str) -> FyrerResult<FyrerConfig> {
        let config: FyrerConfig = serde_yaml::from_str(content)
            .map_err(|e| FyrerError::Config(ConfigError::ParseYaml(e)))?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> FyrerResult<()> {
        self.validate_version()?;
        self.validate_projects()?;
        self.validate_tasks()?;
        Ok(())
    }

    fn validate_version(&self) -> FyrerResult<()> {
        if self.version != 1 {
            return Err(FyrerError::Config(ConfigError::UnsupportedVersion {
                version: self.version,
            }));
        }
        Ok(())
    }

    fn validate_projects(&self) -> FyrerResult<()> {
        let mut project_names = HashSet::new();
        for project in &self.projects {
            if !project_names.insert(&project.name) {
                return Err(FyrerError::Config(ConfigError::DuplicateProject {
                    name: project.name.clone(),
                }));
            }
            if project.root.as_os_str().is_empty() {
                return Err(FyrerError::Config(ConfigError::EmptyProjectRoot {
                    project: project.name.clone(),
                }));
            }
            if project.root.is_absolute() {
                return Err(FyrerError::Config(ConfigError::AbsoluteProjectRoot {
                    project: project.name.clone(),
                    path: project.root.display().to_string(),
                }));
            }
            if project.env_path.is_empty() {
                return Err(FyrerError::Config(ConfigError::EmptyEnvPath {
                    project: project.name.clone(),
                }));
            }
        }
        Ok(())
    }

    fn validate_tasks(&self) -> FyrerResult<()> {
        for project in &self.projects {
            for (task_name, task) in &project.tasks {
                if task.cmd.is_empty() {
                    return Err(FyrerError::Config(ConfigError::EmptyCommand {
                        project: project.name.clone(),
                        task: task_name.clone(),
                    }));
                }

                if task.cache && task.outputs.is_empty() {
                    return Err(FyrerError::Config(ConfigError::CacheWithoutOutputs {
                        project: project.name.clone(),
                        task: task_name.clone(),
                    }));
                }

                if task.restart.strategy == RestartStrategy::FileChange && task.inputs.is_empty() {
                    return Err(FyrerError::Config(ConfigError::FileChangeWithoutInputs {
                        project: project.name.clone(),
                        task: task_name.clone(),
                    }));
                }
            }
        }
        Ok(())
    }

    pub fn create_task_map(&self) -> FyrerResult<TaskMap> {
        let mut task_map = HashMap::new();
        for project in &self.projects {
            let env_path = project.root.join(&project.env_path);
            for (task_name, task_config) in &project.tasks {
                let project_env = merge(&self.env, &project.env);
                let task = Task {
                    project_name: project.name.clone(),
                    project_root: project.root.clone(),
                    env: get_task_env_var(&project_env, &task_config.env, &env_path)?,
                    task_name: task_name.clone(),
                    cmd: task_config.cmd.clone(),
                    depends_on: task_config.depends_on.clone(),
                    inputs: task_config.inputs.clone(),
                    outputs: task_config.outputs.clone(),
                    ignore: task_config.ignore.clone(),
                    cache: task_config.cache,
                    restart: task_config.restart.clone(),
                };
                let task_id = TaskId::new(&project.name, task_name);
                task_map.insert(task_id, task);
            }
        }
        Ok(task_map)
    }
}
fn default_vec_string() -> Vec<String> {
    Vec::new()
}

fn default_env_map() -> EnvMap {
    HashMap::new()
}

fn default_env_path() -> String {
    ".env".to_string()
}

fn default_tasks() -> HashMap<String, TaskConfig> {
    HashMap::new()
}

fn default_bool() -> bool {
    false
}

fn default_cmd() -> String {
    "echo from fyrer".to_string()
}

fn default_restart() -> RestartConfig {
    RestartConfig {
        strategy: RestartStrategy::Never,
        delay: None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{
        config::{FyrerConfig, RestartStrategy},
        error::{FyrerError, config::ConfigError},
    };

    #[test]
    fn test_valid_config() {
        let yaml = r#"
version: 1
env:
  GLOBAL_VAR: global_value
projects:
  - name: project1
    root: ./project1
    env:
      PROJECT_VAR: project_value
    env_path: .env
    tasks:
      build:
        cmd: echo Building project1
        depends_on: []
        inputs: ["src/**/*"]
        outputs: ["dist/**/*"]
        ignore: []
        cache: true
        restart:
          strategy: FileChange
          delay: 1000
      test:
        cmd: "echo Testing project1"
        depends_on: ["build"]
        inputs: ["tests/**/*"]
        outputs: []
        ignore: []
        cache: false
        restart:
          strategy: OnFailure
          delay: 500
  - name: project2
    root: ./project2
    env:
        PROJECT_VAR: project2_value
    env_path: .env
    tasks:
      deploy:
        cmd: "echo Deploying project2"
        depends_on: []
        inputs: []
        outputs: []
        ignore: []
        cache: false
        restart:
            strategy: Never
            delay: null
"#;
        let config = FyrerConfig::new_from_str(yaml).expect("Failed to parse invalid config");
        assert_eq!(config.version, 1);
        assert_eq!(config.env.get("GLOBAL_VAR").unwrap(), "global_value");
        assert_eq!(config.projects.len(), 2);

        let project1 = &config.projects[0];
        assert_eq!(project1.name, "project1");
        assert_eq!(project1.root, PathBuf::from("./project1"));
        assert_eq!(project1.env.get("PROJECT_VAR").unwrap(), "project_value");
        assert_eq!(project1.env_path, ".env");
        assert_eq!(project1.tasks.len(), 2);

        let build_task = project1.tasks.get("build").unwrap();
        assert_eq!(build_task.cmd, "echo Building project1");
        assert_eq!(build_task.depends_on, Vec::<String>::new());
        assert_eq!(build_task.inputs, vec!["src/**/*"]);
        assert_eq!(build_task.outputs, vec!["dist/**/*"]);
        assert_eq!(build_task.ignore, Vec::<String>::new());
        assert!(build_task.cache);
        assert_eq!(build_task.restart.strategy, RestartStrategy::FileChange);
        assert_eq!(build_task.restart.delay, Some(1000));

        let test_task = project1.tasks.get("test").unwrap();
        assert_eq!(test_task.cmd, "echo Testing project1");
        assert_eq!(test_task.depends_on, vec!["build"]);
        assert_eq!(test_task.inputs, vec!["tests/**/*"]);
        assert_eq!(test_task.outputs, Vec::<String>::new());
        assert_eq!(test_task.ignore, Vec::<String>::new());
        assert!(!test_task.cache);
        assert_eq!(test_task.restart.strategy, RestartStrategy::OnFailure);
        assert_eq!(test_task.restart.delay, Some(500));

        let project2 = &config.projects[1];
        assert_eq!(project2.name, "project2");
    }

    #[test]
    fn test_duplicate_project_names() {
        let yaml = r#"
version: 1
projects:
    - name: project1
      root: ./project1
      env_path: .env
      tasks: {}
    - name: project1
      root: ./project2
      env_path: .env
      tasks: {}
"#;
        let err = FyrerConfig::new_from_str(yaml).err().unwrap();
        match err {
            FyrerError::Config(ConfigError::DuplicateProject { name }) => {
                assert_eq!(name, "project1");
            }
            _ => panic!("Expected DuplicateProject error"),
        }
    }

    #[test]
    fn test_invalid_version() {
        let yaml = r#"
version: 2
projects: []
"#;
        let err = FyrerConfig::new_from_str(yaml).err().unwrap();
        match err {
            FyrerError::Config(ConfigError::UnsupportedVersion { version }) => {
                assert_eq!(version, 2);
            }
            _ => panic!("Expected UnsupportedVersion error"),
        }
    }

    #[test]
    fn test_empty_cmd() {
        let yaml = r#"
version: 1
env: {}
projects: 
    - name: project1
      root: ./project1
      env_path: .env
      tasks:
        build:
          cmd: ""
          depends_on: []
          inputs: []
          outputs: []
          ignore: []
          cache: false
          restart:
            strategy: Never
            delay: null
"#;
        let err = FyrerConfig::new_from_str(yaml).err().unwrap();
        match err {
            FyrerError::Config(ConfigError::EmptyCommand { project, task }) => {
                assert_eq!(project, "project1");
                assert_eq!(task, "build");
            }
            _ => panic!("Expected EmptyCommand error"),
        }
    }

    #[test]
    fn test_cache_without_outputs() {
        let yaml = r#"
version: 1
env: {}
projects:
    - name: project1
      root: ./project1
      env_path: .env
      tasks:
        build:
          cmd: echo Building
          depends_on: []
          inputs: []
          outputs: []
          ignore: []
          cache: true
          restart:
            strategy: Never
            delay: null
"#;
        let err = FyrerConfig::new_from_str(yaml).err().unwrap();
        match err {
            FyrerError::Config(ConfigError::CacheWithoutOutputs { project, task }) => {
                assert_eq!(project, "project1");
                assert_eq!(task, "build");
            }
            _ => panic!("Expected CacheWithoutOutputs error"),
        }
    }
}
