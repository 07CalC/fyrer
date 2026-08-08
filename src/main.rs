use anyhow::Result;
use clap::Parser;
use fyrer::{FyrerConfig, TaskId, app::App, cache::cache::get_hash, cli::Cli};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = FyrerConfig::new_from_path(&cli.config)?;
    let task_map = config.create_task_map()?;
    let mut ans = get_hash(
        &task_map.get(&TaskId::new("api", "build")).unwrap(),
        &task_map,
    )?;
    println!("Hash: {:?}", ans);
    // let mut app = App::init(&cli.config)?;
    // app.start(cli.command).await?;
    Ok(())
}

