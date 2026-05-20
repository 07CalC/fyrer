use std::collections::HashMap;

use fyrer_core::tasks::{TaskId, TaskMap};
use fyrer_error::{FyrerResult, graph::GraphError};

#[derive(Debug)]
pub struct TaskGraph {
    pub nodes: HashMap<TaskId, TaskNode>,
}

#[derive(Debug)]
pub struct TaskNode {
    pub id: TaskId,
    pub deps: Vec<TaskId>,
    pub dependents: Vec<TaskId>,
}

impl TaskGraph {
    pub fn new(task_map: &TaskMap) -> TaskGraph {
        let mut graph = TaskGraph {
            nodes: HashMap::new(),
        };

        for (id, _) in task_map {
            graph.nodes.insert(
                id.clone(),
                TaskNode {
                    id: id.clone(),
                    deps: vec![],
                    dependents: Vec::new(),
                },
            );
        }

        for (id, task) in task_map {
            for dep in &task.depends_on {
                let parts: Vec<&str> = dep.split(':').collect();
                let dep_id = TaskId::new(parts[0], parts[1]);
                graph.nodes.get_mut(id).unwrap().deps.push(dep_id.clone());
                graph
                    .nodes
                    .get_mut(&dep_id)
                    .unwrap()
                    .dependents
                    .push(id.clone());
            }
        }
        graph
    }

    pub fn validate(&self) -> FyrerResult<()> {
        let mut visited = HashMap::new();
        for node in self.nodes.values() {
            if !visited.contains_key(&node.id) {
                if self.has_cycle(&node.id, &mut visited) {
                    return Err(fyrer_error::FyrerError::Graph(GraphError::CycleDetected(
                        node.id.to_string(),
                    )));
                }
            }
        }
        Ok(())
    }

    fn has_cycle(&self, node_id: &TaskId, visited: &mut HashMap<TaskId, bool>) -> bool {
        visited.insert(node_id.clone(), true);
        for dep in &self.nodes.get(node_id).unwrap().deps {
            if let Some(&true) = visited.get(dep) {
                return true;
            }
            if !visited.contains_key(dep) && self.has_cycle(dep, visited) {
                return true;
            }
        }
        visited.insert(node_id.clone(), false);
        false
    }
}
