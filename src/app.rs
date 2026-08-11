use anyhow::Result;
use clap::Parser;

use crate::{
    cli::{Cli, Command},
    config::FyrerConfig,
    executor::orchestrator::{self, Orchestrator},
};

pub struct App {
    cli: Cli,
}

impl App {
    pub fn new() -> Self {
        let cli = Cli::parse();
        Self { cli }
    }

    pub async fn run(&self) -> Result<()> {
        let config_path = &self.cli.config;
        let command = &self.cli.command;
        let config = FyrerConfig::new_from_path(config_path)?;
        match command {
            Command::Run { task } => {
                let mut orchestrator = Orchestrator::new(config);
                orchestrator.run(task.as_deref()).await?;
            }
            Command::Plan { task } => {
                let mut orchestrator = Orchestrator::new(config);
                orchestrator.plan(task.as_deref())?;
            }
            Command::List => {
                config.list_tasks();
            }
        }
        Ok(())
    }
}
