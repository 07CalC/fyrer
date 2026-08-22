use std::collections::{HashMap, HashSet, VecDeque};

use crate::id::TaskId;

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

#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error("task {task_id} depends on itself")]
    SelfDependency { task_id: TaskId },
    #[error("task {dependent} depends on missing task {dependency}")]
    MissingDependency { dependent: TaskId, dependency: TaskId },
    #[error("cycle detected involving {task_id}")]
    CycleDetected { task_id: TaskId },
    #[error("task {task_id} not found")]
    TaskNotFound { task_id: TaskId },
}

impl TaskGraph {
    /// Build from an iterator of (TaskId, depends_on). Caller provides specs registry.
    pub fn from_specs<I>(specs: I) -> Result<Self, GraphError>
    where
        I: IntoIterator<Item = (TaskId, Vec<TaskId>)>,
    {
        let entries: Vec<_> = specs.into_iter().collect();
        let mut nodes: HashMap<TaskId, TaskNode> = entries
            .iter()
            .map(|(id, _)| {
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
        for (id, deps) in entries {
            for dep in deps {
                if dep == id {
                    return Err(GraphError::SelfDependency { task_id: id });
                }
                if !nodes.contains_key(&dep) {
                    return Err(GraphError::MissingDependency {
                        dependent: id.clone(),
                        dependency: dep.clone(),
                    });
                }
                nodes.get_mut(&id).unwrap().deps.push(dep.clone());
                nodes.get_mut(&dep).unwrap().dependents.push(id.clone());
            }
        }
        Ok(Self { nodes })
    }

    /// Validate for cycles.
    pub fn validate(&self) -> Result<(), GraphError> {
        let mut states = HashMap::new();
        for node in self.nodes.values() {
            if !matches!(states.get(&node.id), Some(VisitState::Done))
                && self.has_cycle(&node.id, &mut states)
            {
                return Err(GraphError::CycleDetected {
                    task_id: node.id.clone(),
                });
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

    pub fn contains(&self, id: &TaskId) -> bool {
        self.nodes.contains_key(id)
    }

    pub fn dependents_of(&self, id: &TaskId) -> Vec<TaskId> {
        self.nodes
            .get(id)
            .map(|n| n.dependents.clone())
            .unwrap_or_default()
    }

    pub fn deps_of(&self, id: &TaskId) -> Vec<TaskId> {
        self.nodes
            .get(id)
            .map(|n| n.deps.clone())
            .unwrap_or_default()
    }

    pub fn transitive_closure(&self, roots: &[TaskId]) -> HashSet<TaskId> {
        let mut relevant = HashSet::new();
        let mut stack = roots.to_vec();
        while let Some(id) = stack.pop() {
            if relevant.insert(id.clone()) {
                if let Some(node) = self.nodes.get(&id) {
                    stack.extend(node.deps.iter().cloned());
                }
            }
        }
        relevant
    }

    pub fn transitive_dependents(&self, id: &TaskId) -> HashSet<TaskId> {
        let mut out = HashSet::new();
        let mut stack = vec![id.clone()];
        let mut seen = HashSet::new();
        seen.insert(id.clone());
        while let Some(cur) = stack.pop() {
            if let Some(node) = self.nodes.get(&cur) {
                for dep in &node.dependents {
                    if seen.insert(dep.clone()) {
                        out.insert(dep.clone());
                        stack.push(dep.clone());
                    }
                }
            }
        }
        out
    }

    /// Legacy level-based ordering (kept for `plan` command).
    pub fn get_levels(&self, tasks: &[TaskId]) -> Result<Vec<Vec<TaskId>>, GraphError> {
        for id in tasks {
            if !self.nodes.contains_key(id) {
                return Err(GraphError::TaskNotFound {
                    task_id: id.clone(),
                });
            }
        }
        let relevant = self.transitive_closure(tasks);
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
            .filter(|(_, d)| **d == 0)
            .map(|(id, _)| id.clone())
            .collect();
        let mut levels = Vec::new();
        let mut processed = HashSet::new();
        while !queue.is_empty() {
            levels.push(queue.iter().cloned().collect());
            let mut next = VecDeque::new();
            for id in &queue {
                processed.insert(id.clone());
                for dep in &self.nodes[id].dependents {
                    if !relevant.contains(dep) {
                        continue;
                    }
                    let deg = in_degree.get_mut(dep).unwrap();
                    *deg -= 1;
                    if *deg == 0 && !processed.contains(dep) {
                        next.push_back(dep.clone());
                    }
                }
            }
            queue = next;
        }
        Ok(levels)
    }

    /// For scheduler seeding: map of TaskId -> pending dep count within closure.
    pub fn in_degree_map(&self, relevant: &HashSet<TaskId>) -> HashMap<TaskId, usize> {
        relevant
            .iter()
            .map(|id| {
                let count = self.nodes[id]
                    .deps
                    .iter()
                    .filter(|d| relevant.contains(*d))
                    .count();
                (id.clone(), count)
            })
            .collect()
    }
}
