use anyhow::Result;

use crate::config::parser::load_config;

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
    Ok(())
}
