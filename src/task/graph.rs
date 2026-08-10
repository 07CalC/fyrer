use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::Result;

use crate::task::{error::GraphError, id::TaskId, map::TaskMap};

#[derive(Debug, Clone)]
pub struct TaskGraph {
    nodes: HashMap<TaskId, TaskNode>,
}

#[derive(Debug, Clone)]
struct TaskNode {
    id: TaskId,
    deps: Vec<TaskId>,
    dependents: Vec<TaskId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Visiting,
    Done,
}

impl TaskGraph {
    pub fn new(task_map: TaskMap) -> Result<Self> {
        let mut nodes: HashMap<TaskId, TaskNode> = task_map
            .tasks
            .keys()
            .map(|id| {
                (
                    id.clone(),
                    TaskNode {
                        id: id.clone(),
                        deps: Vec::new(),
                        dependents: Vec::new(),
                    },
                )
            })
            .collect();

        for (id, task) in task_map.tasks.iter() {
            for dep_id in &task.depends_on {
                if dep_id == id {
                    return Err(GraphError::SelfDependency {
                        task_id: id.clone(),
                    }
                    .into());
                }

                if !nodes.contains_key(&dep_id) {
                    return Err(GraphError::MissingDependency {
                        dependent: id.clone(),
                        dependency: dep_id.clone(),
                    }
                    .into());
                }

                let node = nodes.get_mut(id).expect("task id exists in the graph");
                node.deps.push(dep_id.clone());

                let dependent = nodes
                    .get_mut(&dep_id)
                    .expect("dependency exists in the graph");
                dependent.dependents.push(id.clone());
            }
        }

        Ok(Self { nodes })
    }

    pub fn validate(&self) -> Result<()> {
        let mut states = HashMap::new();
        for node in self.nodes.values() {
            if !matches!(states.get(&node.id), Some(VisitState::Done))
                && self.has_cycle(&node.id, &mut states)
            {
                return Err(GraphError::CycleDetected {
                    task_id: node.id.clone(),
                }
                .into());
            }
        }
        Ok(())
    }

    fn has_cycle(&self, node_id: &TaskId, states: &mut HashMap<TaskId, VisitState>) -> bool {
        match states.get(node_id) {
            Some(VisitState::Visiting) => return true,
            Some(VisitState::Done) => return false,
            None => {}
        }

        states.insert(node_id.clone(), VisitState::Visiting);
        let node = &self.nodes[node_id];
        let has_cycle = node.deps.iter().any(|dep| self.has_cycle(dep, states));
        states.insert(node_id.clone(), VisitState::Done);
        has_cycle
    }

    pub fn get_orders(&self, tasks: &[TaskId]) -> Result<Vec<Vec<(TaskId, Vec<TaskId>)>>> {
        for id in tasks {
            if !self.nodes.contains_key(id) {
                return Err(GraphError::TaskNotFound {
                    task_id: id.clone(),
                }
                .into());
            }
        }

        // Collect the transitive closure of the requested tasks.
        let mut relevant = HashSet::new();
        let mut stack = tasks.to_vec();
        while let Some(id) = stack.pop() {
            if relevant.insert(id.clone()) {
                stack.extend(self.nodes[&id].deps.iter().cloned());
            }
        }

        let deps: HashMap<TaskId, Vec<TaskId>> = relevant
            .iter()
            .map(|id| {
                let ds: Vec<TaskId> = self.nodes[id]
                    .deps
                    .iter()
                    .filter(|dep| relevant.contains(*dep))
                    .cloned()
                    .collect();
                (id.clone(), ds)
            })
            .collect();

        let mut in_degree: HashMap<TaskId, usize> =
            deps.iter().map(|(id, ds)| (id.clone(), ds.len())).collect();

        let mut queue: VecDeque<TaskId> = in_degree
            .iter()
            .filter(|(_, degree)| **degree == 0)
            .map(|(id, _)| id.clone())
            .collect();

        let mut levels = Vec::new();
        let mut processed = HashSet::new();

        while !queue.is_empty() {
            levels.push(
                queue
                    .iter()
                    .map(|id| {
                        let deps = deps[&id].clone();
                        (id.clone(), deps)
                    })
                    .collect(),
            );

            let mut next_queue = VecDeque::new();
            for id in &queue {
                processed.insert(id.clone());
                for dependent in &self.nodes[id].dependents {
                    if !relevant.contains(dependent) {
                        continue;
                    }
                    let degree = in_degree.get_mut(dependent).expect("dependent is relevant");
                    *degree -= 1;
                    if *degree == 0 && !processed.contains(dependent) {
                        next_queue.push_back(dependent.clone());
                    }
                }
            }
            queue = next_queue;
        }

        Ok(levels)
    }
}
