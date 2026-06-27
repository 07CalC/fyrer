use std::{collections::HashMap, fs};

use fyrer_core::config::FyrerConfig;
use fyrer_error::FyrerResult;
fn main() -> FyrerResult<()> {
    let config_str = fs::read_to_string("fyrer.yml").expect("Failed to read config file");
    let config = FyrerConfig::new_from_str(&config_str)?;
    let task_map = config.create_task_map();
    drop(config);
    let task_graph = fyrer_graph::TaskGraph::new(&task_map)?;
    task_graph.validate()?;
    let mut done_map: HashMap<String, bool> = HashMap::new();
    // exec("project1:test", &task_graph, &mut done_map, &task_map).unwrap();
    let order = task_graph.get_order("project1:test".to_string())?;
    println!("Execution order for project1:test: {:?}", order);
    Ok(())
}

fn exec(
    task_name: &str,
    task_graph: &fyrer_graph::TaskGraph,
    done_map: &mut HashMap<String, bool>,
    task_map: &fyrer_core::tasks::TaskMap,
) -> FyrerResult<()> {
    let task = task_graph.get_task(task_name)?;
    for dep in &task.deps {
        if !done_map.contains_key(&dep.to_string()) {
            exec(&dep.to_string(), task_graph, done_map, task_map)?;
        }
    }
    let task = task_map.get(&task.id).unwrap();
    task.execute();
    done_map.insert(task.get_id().to_string(), true);
    Ok(())
}
