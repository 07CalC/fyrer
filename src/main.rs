use fyrer::config::FyrerConfig;
use fyrer::error::FyrerResult;
use fyrer::executor;
use fyrer::global::{self};
use fyrer::graph::TaskGraph;
use std::fs;
#[tokio::main]
async fn main() -> FyrerResult<()> {
    let config_str = fs::read_to_string("fyrer.yml").expect("Failed to read config file");
    let config = FyrerConfig::new_from_str(&config_str)?;
    let task_map = config.create_task_map();
    let task_graph = TaskGraph::new(&task_map)?;
    task_graph.validate()?;
    global::init(task_graph, task_map);
    executor::execute_tasks("web:build").await?;
    Ok(())
}
