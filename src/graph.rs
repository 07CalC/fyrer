use std::collections::{HashMap, HashSet, VecDeque};

use crate::error::{FyrerResult, graph::GraphError};
use crate::tasks::{Task, TaskId, TaskMap};

#[derive(Debug)]
pub struct TaskGraph {
    pub nodes: HashMap<TaskId, TaskNode>,
}

#[derive(Debug)]
pub struct TaskNode {
    pub id: TaskId,
    pub task: Task,
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
                    task: task_map.get(&id).unwrap().clone(),
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
                    return Err(crate::error::FyrerError::Graph(GraphError::SelfDependency(
                        id.to_string(),
                    )));
                }

                if !graph.nodes.contains_key(&dep_id) {
                    return Err(crate::error::FyrerError::Graph(
                        GraphError::MissingDependency {
                            dependent: id.to_string(),
                            dependency: dep_id.to_string(),
                        },
                    ));
                }

                graph.nodes.get_mut(&id).unwrap().deps.push(dep_id.clone());
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
                    return Err(crate::error::FyrerError::Graph(GraphError::CycleDetected(
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
    pub fn get_order(&self, task: &str) -> FyrerResult<Vec<Vec<TaskId>>> {
        let task_id = TaskId::from_string(&task).ok_or_else(|| {
            crate::error::FyrerError::Graph(GraphError::InvalidTaskId {
                dependency: task.to_string(),
                task: task.to_string(),
            })
        })?;

        let mut relevant = HashSet::new();
        let mut stack = vec![task_id];
        while let Some(id) = stack.pop() {
            if relevant.insert(id.clone()) {
                let node = self.nodes.get(&id).ok_or_else(|| {
                    crate::error::FyrerError::Graph(GraphError::MissingDependency {
                        dependent: task.to_string(),
                        dependency: id.to_string(),
                    })
                })?;
                stack.extend(node.deps.iter().cloned());
            }
        }

        let mut in_degree: HashMap<&TaskId, usize> = relevant.iter().map(|id| (id, 0)).collect();
        for id in &relevant {
            let node = self.nodes.get(id).unwrap();
            for dep in &node.deps {
                if relevant.contains(dep) {
                    *in_degree.get_mut(id).unwrap() += 1;
                }
            }
        }

        let mut queue: VecDeque<&TaskId> = in_degree
            .iter()
            .filter(|&(_, deg)| *deg == 0)
            .map(|(id, _)| *id)
            .collect();

        let mut levels = Vec::new();
        let mut processed = HashSet::new();

        while !queue.is_empty() {
            levels.push(
                queue
                    .iter()
                    .map(|id| self.nodes.get(id).unwrap().id.clone())
                    .collect(),
            );

            let mut next_queue = VecDeque::new();
            for id in &queue {
                processed.insert((*id).clone());
                let node = self.nodes.get(id).unwrap();
                for dependent in &node.dependents {
                    if !relevant.contains(dependent) {
                        continue;
                    }
                    if let Some(deg) = in_degree.get_mut(dependent) {
                        *deg -= 1;
                        if *deg == 0 && !processed.contains(dependent) {
                            next_queue.push_back(dependent);
                        }
                    }
                }
            }
            queue = next_queue;
        }

        Ok(levels)
    }

    pub fn get_task(&self, task_name: &str) -> FyrerResult<&Task> {
        let task_id = TaskId::from_string(task_name).ok_or_else(|| {
            crate::error::FyrerError::Graph(GraphError::InvalidTaskId {
                dependency: task_name.to_string(),
                task: task_name.to_string(),
            })
        })?;

        match self.nodes.get(&task_id) {
            Some(node) => Ok(&node.task),
            None => Err(crate::error::FyrerError::Graph(
                GraphError::MissingDependency {
                    dependent: task_name.to_string(),
                    dependency: task_name.to_string(),
                },
            )),
        }
    }
}
