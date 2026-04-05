use serde_derive::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct FyrerConfig {
    pub projects: Vec<Project>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    pub name: String,
    pub root: String,
    pub setup: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub env_path: Option<String>,
    pub tasks: HashMap<String, Task>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Task {
    pub dir: Option<String>,
    pub cmd: String,
    pub watch: String,
    pub env: Option<HashMap<String, String>>,
    pub env_path: Option<String>,
    pub quiet: Option<bool>,
}
