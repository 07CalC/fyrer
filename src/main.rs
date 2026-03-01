use colored::Colorize;

use crate::{
    cli::{logger::Logger, print_banner::print_banner},
    colors::COLORS,
    parser::load_config,
};

mod cli;
mod colors;
mod config;
mod env_parser;
mod installer;
mod kill_process;
mod parser;
mod runner;
mod spawn_service;
mod tui;
mod watcher;

#[tokio::main]
async fn main() {
    clearscreen::clear().expect("Failed to clear the screen");
    let config = load_config("fyrer.yml");
    let is_tui = config.tui.unwrap_or(false);
    if is_tui {
    } else {
        print_banner();
        installer::run_installers(&config).await;
        println!("{} {}", "┌─", "Starting services...".bright_cyan().bold());
        println!("{}", "│");

        let max_name_len = config
            .services
            .iter()
            .map(|s| s.name.len() + 2) // +2 for brackets [ ]
            .max()
            .unwrap_or(8); // default if no services

        let logger = Logger::new(config.services.len(), max_name_len);

        for (i, service) in config.services.into_iter().enumerate() {
            let color = COLORS[i % COLORS.len()];
            tokio::spawn(runner::runner(service, color, logger.sender()));
        }
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for Ctrl+C");
        println!(
            "\n{} {}",
            "└─",
            "Received Ctrl+C, shutting down...".bright_cyan().bold()
        );
    }
}
