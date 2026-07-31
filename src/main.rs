use std::fs;

use fyrer::config::FyrerConfig;
use fyrer::error::FyrerResult;
use fyrer::graph::TaskGraph;

#[tokio::main]
async fn main() -> FyrerResult<()> {
    let config_str = fs::read_to_string("fyrer.yml").expect("Failed to read config file");
    let config = FyrerConfig::new_from_str(&config_str)?;
    let task_map = config.create_task_map();
    let task_graph = TaskGraph::new(task_map)?;
    task_graph.validate()?;
    let order = task_graph.get_order("web:build".to_string())?;
    println!("Execution order for task 'web:build': {:?}", order);
    Ok(())
}
