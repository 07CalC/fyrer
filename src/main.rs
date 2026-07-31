use std::fs;

use fyrer::config::FyrerConfig;
use fyrer::error::FyrerResult;
use fyrer::graph::TaskGraph;
use fyrer::tasks::{TaskId, TaskMap};

#[tokio::main]
async fn main() -> FyrerResult<()> {
    let config_str = fs::read_to_string("fyrer.yml").expect("Failed to read config file");
    let config = FyrerConfig::new_from_str(&config_str)?;
    let task_map = config.create_task_map();
    drop(config);
    let task_graph = TaskGraph::new(&task_map)?;
    task_graph.validate()?;
    let order = task_graph.get_order("project1:test".to_string())?;
    exec(order, &task_map).await;
    Ok(())
}

async fn exec(order: Vec<Vec<TaskId>>, task_map: &TaskMap) {
    for batch in order {
        let handles: Vec<_> = batch
            .into_iter()
            .filter_map(|id| {
                let task = task_map.get(&id)?.clone();
                Some(tokio::task::spawn_blocking(move || task.execute()))
            })
            .collect();

        for handle in handles {
            let _ = handle.await;
        }
    }
}
