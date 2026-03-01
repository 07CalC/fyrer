use colored::Colorize;

use crate::{colors::COLORS, parser::load_config};

mod colors;
mod config;
mod env_parser;
mod installer;
mod kill_process;
mod parser;
mod print_banner;
mod runner;
mod spawn_service;
mod watcher;

#[tokio::main]
async fn main() {
    clearscreen::clear().expect("Failed to clear the screen");
    let config = load_config("fyrer.yml");
    let mut handles = vec![];
    print_banner::print_banner();
    installer::run_installers(&config).await;
    println!("{} {}", "┌─", "Starting services...".bright_cyan().bold());

    println!("{}", "│".bright_black());

    let max_name_len = config
        .services
        .iter()
        .map(|s| s.name.len() + 2) // +2 for brackets [ ]
        .max()
        .unwrap_or(8); // default if no services

    for (i, service) in config.services.into_iter().enumerate() {
        let color = COLORS[i % COLORS.len()];
        let handle = tokio::spawn(runner::runner(service, color, max_name_len));
        handles.push(handle);
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
