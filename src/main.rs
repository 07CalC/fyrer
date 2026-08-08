use std::{path::PathBuf, str::FromStr};

use anyhow::Result;
use clap::Parser;
use fyrer::{
    FyrerConfig, TaskId,
    app::App,
    cache::{
        CacheMetadata, cache::get_hash, cache_provider::CacheProvider, local::LocalCacheProvider,
    },
    cli::Cli,
};

#[tokio::main]
async fn main() -> Result<()> {
    // let cli = Cli::parse();
    // let config = FyrerConfig::new_from_path(&cli.config)?;
    // let task_map = config.create_task_map()?;
    // let mut ans = get_hash(
    //     &task_map.get(&TaskId::new("api", "build")).unwrap(),
    //     &task_map,
    // )?;
    // println!("Hash: {:?}", ans);
    // let mut app = App::init(&cli.config)?;
    // app.start(cli.command).await?;
    //

    let local_cache = LocalCacheProvider::default();
    // local_cache.save(
    //     "2398nasdfklj34kj23lkj4",
    //     &vec![PathBuf::from_str("examples/bun-http/dist").unwrap()],
    //     CacheMetadata {
    //         task: "api:build".to_string(),
    //         hash: "2398nasdfklj34kj23lkj4".to_string(),
    //         cmd: "bun build".to_string(),
    //         dependencies: vec!["api:install".to_string()],
    //         duration_ms: 1234,
    //         exit_code: 0,
    //         outputs: vec!["examples/bun-http/dist".to_string()],
    //         cache: fyrer::cache::CacheStatus::Hit,
    //         cache_key: Some("2398nasdfklj34kj23lkj4".to_string()),
    //         timestamp: 1234567890,
    //     },
    // )?;
    local_cache.restore("2398nasdfklj34kj23lkj4")?;
    Ok(())
}
