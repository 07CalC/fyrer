use anyhow::Result;
use clap::Parser;

use crate::{
    cli::{Cli, Command},
    config::FyrerConfig,
};

pub struct App {
    cli: Cli,
}

impl App {
    pub fn new() -> Self {
        let cli = Cli::parse();
        Self { cli }
    }

    pub fn run(&self) -> Result<()> {
        let config_path = &self.cli.config;
        let command = &self.cli.command;
        let config = FyrerConfig::new_from_path(config_path)?;
        match command {
            Command::Run {
                task,
                dry_run,
                no_tui,
            } => {}
            Command::List => {
                config.list_tasks();
            }
        }
        Ok(())
    }
}
