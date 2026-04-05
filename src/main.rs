use std::collections::HashMap;

use anyhow::Result;

use crate::config::{parser::load_config, types::Project};

mod config;
mod core;
mod env;
mod error;
mod kill_process;
mod print_banner;

#[tokio::main]
async fn main() -> Result<()> {
    let content = load_config("fyrer.yml")?;
    clearscreen::clear().expect("Failed to clear the screen");
    print_banner::print_banner();
    let project1 = Project {
        name: "crux".into(),
        root: "../crux/".into(),
        env: None,
        env_path: None,
        setup: Some("bun install".into()),
        tasks: HashMap::new(),
    };
    let project = Project {
        name: "fyrer".into(),
        root: ".".into(),
        env: None,
        env_path: None,
        setup: Some("cargo fetch".into()),
        tasks: HashMap::new(),
    };

    let projects = vec![project, project1];
    for project in projects {
        project.setup();
    }

    Ok(())
}
