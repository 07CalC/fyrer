use fyrer::config::FyrerConfig;
use fyrer::error::FyrerResult;
use fyrer::executor;
use fyrer::global::{self};
use fyrer::graph::TaskGraph;
use fyrer::logger::Logger;
use std::fs;
#[tokio::main]
async fn main() -> FyrerResult<()> {
    let config_str = fs::read_to_string("fyrer.yml").expect("Failed to read config file");
    let config = FyrerConfig::new_from_str(&config_str)?;
    let task_map = config.create_task_map();
    let task_graph = TaskGraph::new(&task_map)?;
    task_graph.validate()?;
    let mut logger = Logger::new(task_map.len());
    let log_sender = logger.add_task();
    tokio::spawn(async move {
        logger.start().await;
    });
    global::init(task_graph, task_map, config.env, log_sender);
    executor::execute_tasks("web:build").await?;
    Ok(())
}
