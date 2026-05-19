use std::fs;

use fyrer_core::{config::FyrerConfig, error::FyrerResult};
fn main() -> FyrerResult<()> {
    let config_str = fs::read_to_string("fyrer.yml").expect("Failed to read config file");
    let config = FyrerConfig::new_from_str(&config_str)?;
    let task_map = config.create_task_map();
    drop(config);
    let task_graph = fyrer_graph::TaskGraph::new(&task_map);
    dbg!(task_graph.validate().unwrap());
    Ok(())
}
