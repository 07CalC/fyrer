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
    pub fn new(task_map: &TaskMap) -> FyrerResult<Self> {
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
                let dep_id = if let Some((proj, task_name)) = dep.split_once(':') {
                    TaskId::new(proj, task_name)
                } else {
                    TaskId::new(&task.project_name, dep)
                };

                if dep_id == *id {
                    return Err(fyrer_error::FyrerError::Graph(
                        GraphError::SelfDependency(id.to_string()),
                    ));
                }

                if !graph.nodes.contains_key(&dep_id) {
                    return Err(fyrer_error::FyrerError::Graph(
                        GraphError::MissingDependency {
                            dependent: id.to_string(),
                            dependency: dep_id.to_string(),
                        },
                    ));
                }

                graph.nodes.get_mut(id).unwrap().deps.push(dep_id.clone());
                graph
                    .nodes
                    .get_mut(&dep_id)
                    .unwrap()
                    .dependents
                    .push(id.clone());
            }
        }
        Ok(graph)
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

    pub fn get_exec_flow(&self, task: String) -> FyrerResult<Vec<Vec<TaskId>>> {
        let mut flow = Vec::new();
        let task_id = TaskId::from_string(&task);

        match task_id {
            Some(id) => {
                self.build_flow(&id, &mut flow)?;
            }
            None => {
                return Err(fyrer_error::FyrerError::Graph(GraphError::InvalidTaskId {
                    dependency: task.clone(),
                    task: task.clone(),
                }));
            }
        }

        Ok(flow)
    }
    fn build_flow(&self, task_id: &TaskId, flow: &mut Vec<Vec<TaskId>>) -> FyrerResult<()> {
        if let Some(node) = self.nodes.get(task_id) {
            for dep in &node.deps {
                self.build_flow(dep, flow)?;
            }
            flow.push(vec![task_id.clone()]);
        } else {
            return Err(fyrer_error::FyrerError::Graph(
                GraphError::MissingDependency {
                    dependent: task_id.to_string(),
                    dependency: task_id.to_string(),
                },
            ));
        }
        Ok(())
    }
}
