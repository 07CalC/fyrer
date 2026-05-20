use fyrer_error::{FyrerError, FyrerResult, config::ConfigError};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use crate::tasks::{Task, TaskId, TaskMap};

pub type EnvMap = HashMap<String, String>;

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
    #[serde(default = "default_bool")]
    pub persistent: bool,
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
        config
            .validate()
            .map_err(|e| FyrerError::Config(ConfigError::InvalidConfig(e.to_string())))?;
        Ok(config)
    }

    pub fn new_from_str(content: &str) -> FyrerResult<FyrerConfig> {
        let config: FyrerConfig = serde_yaml::from_str(content)
            .map_err(|e| FyrerError::Config(ConfigError::ParseYaml(e)))?;
        config
            .validate()
            .map_err(|e| FyrerError::Config(ConfigError::InvalidConfig(e.to_string())))?;
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
            return Err(FyrerError::Config(ConfigError::InvalidConfig(format!(
                "unsupported config version: {}",
                self.version
            ))));
        }
        Ok(())
    }

    fn validate_projects(&self) -> FyrerResult<()> {
        let mut project_names = HashSet::new();
        for project in &self.projects {
            // let mut task_names: HashSet<String> = HashSet::new();
            if !project_names.insert(&project.name) {
                return Err(FyrerError::Config(ConfigError::InvalidConfig(format!(
                    "duplicate project name: {}",
                    project.name
                ))));
            }
            if project.root.as_os_str().is_empty() {
                return Err(FyrerError::Config(ConfigError::InvalidConfig(format!(
                    "project '{}' has empty root path",
                    project.name
                ))));
            }
            if project.root.is_absolute() {
                return Err(FyrerError::Config(ConfigError::InvalidConfig(format!(
                    "project '{}' has absolute root path '{}'",
                    project.name,
                    project.root.display()
                ))));
            }
            if project.env_path.is_empty() {
                return Err(FyrerError::Config(ConfigError::InvalidConfig(format!(
                    "project '{}' has empty env_path",
                    project.name
                ))));
            }

            //TODO: currently we are making a map of tasks, so its not possible to detect duplicate
            //task names at the yaml parsing stage. We need to add custom validation to check for
            //duplicate task names and return an error if found. This is a known issue that we will
            //address in a future update.
            //
            // for task_name in project.tasks.keys() {
            //     if !task_names.insert(task_name) {
            //         return Err(FyrerError::Config(ConfigError::InvalidConfig(format!(
            //             "duplicate task name '{}' in project '{}'",
            //             task_name, project.name
            //         ))));
            //     }
            // }
        }
        Ok(())
    }

    fn validate_tasks(&self) -> FyrerResult<()> {
        for project in &self.projects {
            for (task_name, task) in &project.tasks {
                if task.cmd.is_empty() {
                    return Err(FyrerError::Config(ConfigError::InvalidConfig(format!(
                        "task '{}' in project '{}' has empty cmd",
                        task_name, project.name
                    ))));
                }

                if task.cache && task.outputs.is_empty() {
                    return Err(FyrerError::Config(ConfigError::InvalidConfig(format!(
                        "task '{}' in project '{}' has cache enabled but no outputs defined",
                        task_name, project.name
                    ))));
                }

                if task.cache && task.persistent {
                    return Err(FyrerError::Config(ConfigError::InvalidConfig(format!(
                        "task '{}' in project '{}' cannot be both cacheable and persistent",
                        task_name, project.name
                    ))));
                }

                if task.restart.strategy == RestartStrategy::FileChange && task.inputs.is_empty() {
                    return Err(FyrerError::Config(ConfigError::InvalidConfig(format!(
                        "task '{}' in project '{}' has file change restart strategy but no inputs defined",
                        task_name, project.name
                    ))));
                }
            }
        }
        Ok(())
    }

    pub fn create_task_map(&self) -> TaskMap {
        let mut task_map = HashMap::new();
        for project in &self.projects {
            for (task_name, task_config) in &project.tasks {
                let mut env = self.env.clone();
                env.extend(project.env.clone());
                env.extend(task_config.env.clone());
                let task = Task {
                    project_name: project.name.clone(),
                    project_root: project.root.clone(),
                    env,
                    task_name: task_name.clone(),
                    cmd: task_config.cmd.clone(),
                    depends_on: task_config.depends_on.clone(),
                    persistent: task_config.persistent,
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
        task_map
    }
}
fn default_vec_string() -> Vec<String> {
    Vec::new()
}

fn default_env_map() -> EnvMap {
    HashMap::new()
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

mod tests {
    use super::*;
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
        persistent: false
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
        persistent: false
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
        persistent: true
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
        assert!(!build_task.persistent);
        assert_eq!(build_task.inputs, vec!["src/**/*"]);
        assert_eq!(build_task.outputs, vec!["dist/**/*"]);
        assert_eq!(build_task.ignore, Vec::<String>::new());
        assert!(build_task.cache);
        assert_eq!(build_task.restart.strategy, RestartStrategy::FileChange);
        assert_eq!(build_task.restart.delay, Some(1000));

        let test_task = project1.tasks.get("test").unwrap();
        assert_eq!(test_task.cmd, "echo Testing project1");
        assert_eq!(test_task.depends_on, vec!["build"]);
        assert!(!test_task.persistent);
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
            FyrerError::Config(ConfigError::InvalidConfig(msg)) => {
                assert!(msg.contains("duplicate project name: project1"));
            }
            _ => panic!("Expected InvalidConfig error"),
        }
    }

    #[test]
    #[ignore = "This test currently fails because serde_yaml allows duplicate keys and we need to add custom validation to detect them. This is a known issue that we will address in a future update."]
    fn test_duplicate_task_names() {
        let yaml = r#"
version: 1
projects:
    - name: project1
      root: ./project1
      env_path: .env
      tasks:
        build:
          cmd: echo Building
          depends_on: []
          persistent: false
          inputs: []
          outputs: []
          ignore: []
          cache: false
          restart:
            strategy: Never
            delay: null
        build:
          cmd: echo Building again
          depends_on: []
          persistent: false
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
            FyrerError::Config(ConfigError::InvalidConfig(msg)) => {
                assert!(msg.contains("duplicate task name 'build' in project 'project1'"));
            }
            _ => panic!("Expected InvalidConfig error"),
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
            FyrerError::Config(ConfigError::InvalidConfig(msg)) => {
                assert!(msg.contains("unsupported config version: 2"));
            }
            _ => panic!("Expected InvalidConfig error"),
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
          persistent: false
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
            FyrerError::Config(ConfigError::InvalidConfig(msg)) => {
                assert!(msg.contains("task 'build' in project 'project1' has empty cmd"));
            }
            _ => panic!("Expected InvalidConfig error"),
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
          persistent: false
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
            FyrerError::Config(ConfigError::InvalidConfig(msg)) => {
                dbg!(&msg);
                assert!(msg.contains(
                    "task 'build' in project 'project1' has cache enabled but no outputs defined"
                ));
            }
            _ => panic!("Expected InvalidConfig error"),
        }
    }

    #[test]
    fn test_cache_and_persistent() {
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
            persistent: true
            inputs: []
            outputs: ["dist/**/*"]
            ignore: []
            cache: true
            restart:
                strategy: Never
                delay: null
"#;
        let err = FyrerConfig::new_from_str(yaml).err().unwrap();
        match err {
            FyrerError::Config(ConfigError::InvalidConfig(msg)) => {
                assert!(msg.contains(
                    "task 'build' in project 'project1' cannot be both cacheable and persistent"
                ));
            }
            _ => panic!("Expected InvalidConfig error"),
        }
    }
}
