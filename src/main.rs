use std::collections::HashMap;

use colored::Colorize;

use crate::config::Project;

mod cli;
mod colors;
mod config;
mod core;
mod executor;
mod kill_process;
mod logger;
mod print_banner;
mod utils;

#[tokio::main]
async fn main() {
    clearscreen::clear().expect("Failed to clear the screen");
    // let config = load_config("fyrer.yml");
    print_banner::print_banner();
    let project = Project {
        name: "crux".to_string(),
        env_path: None,
        env: None,
        setup: Some("bun install".to_string()),
        root: "/home/calc/Documents/crux/".to_string(),
        tasks: HashMap::new(),
    };
    if let Err(error) = project.setup().await {
        eprintln!("{error}");
        return;
    }
}
