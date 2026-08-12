use std::sync::Arc;

use anyhow::Result;
use clap::Parser;

use crate::{
    cli::{Cli, Command},
    config::FyrerConfig,
    executor::orchestrator::{self, Orchestrator},
    ui::{plain::PlainUi, tui::Tui},
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
            Command::Run { task, no_tui } => {
                let mut orchestrator = Orchestrator::new(config);
                if *no_tui {
                    let ui = PlainUi::default();
                    orchestrator.run(task.as_deref(), ui).await?;
                } else {
                    let ui = Tui::new();
                    orchestrator.run(task.as_deref(), ui).await?;
                }
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
