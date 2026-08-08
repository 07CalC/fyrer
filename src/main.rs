use anyhow::Result;
use clap::Parser;
use fyrer::{app::App, cli::Cli};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut app = App::init(&cli.config)?;
    app.start(cli.command).await?;
    Ok(())
}
