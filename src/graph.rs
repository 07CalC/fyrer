//! Dependency graph of tasks and topological ordering.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::{
    config::TaskMap,
    error::{FyrerError, FyrerResult, GraphError},
    tasks::TaskId,
};

/// A directed acyclic graph of tasks.
#[derive(Debug, Clone)]
pub struct TaskGraph {
    nodes: HashMap<TaskId, TaskNode>,
}

/// A node in the task graph.
#[derive(Debug, Clone)]
struct TaskNode {
    id: TaskId,
    deps: Vec<TaskId>,
    dependents: Vec<TaskId>,
}

/// Visit state used while detecting cycles with depth-first search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisitState {
    /// The node is currently on the DFS stack.
    Visiting,
    /// The node has been fully explored.
    Done,
}

impl TaskGraph {
    /// Builds a graph from a task map, resolving all `depends_on` edges.
    ///
    /// # Errors
    ///
    /// Returns an error if a task depends on itself or on a task that does
    /// not exist.
    ///
    /// # Panics
    ///
    /// Panics if the task map is internally inconsistent.
    pub fn new(task_map: &TaskMap) -> FyrerResult<Self> {
        let mut nodes: HashMap<TaskId, TaskNode> = task_map
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

        for (id, task) in task_map {
            for dep in &task.depends_on {
                let dep_id = match dep.split_once(':') {
                    Some((project, task)) => TaskId::new(project, task),
                    None => TaskId::new(&task.project_name, dep),
                };

                if dep_id == *id {
                    return Err(FyrerError::Graph(GraphError::SelfDependency(
                        id.to_string(),
                    )));
                }

                if !nodes.contains_key(&dep_id) {
                    return Err(FyrerError::Graph(GraphError::MissingDependency {
                        dependent: id.to_string(),
                        dependency: dep_id.to_string(),
                    }));
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

    /// Returns an error if the graph contains a dependency cycle.
    ///
    /// # Errors
    ///
    /// Returns an error if a cycle is detected.
    ///
    /// # Panics
    ///
    /// Panics if the graph is internally inconsistent.
    pub fn validate(&self) -> FyrerResult<()> {
        let mut states = HashMap::new();
        for node in self.nodes.values() {
            if !matches!(states.get(&node.id), Some(VisitState::Done))
                && self.has_cycle(&node.id, &mut states)
            {
                return Err(FyrerError::Graph(GraphError::CycleDetected(
                    node.id.to_string(),
                )));
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

    /// Returns the given tasks and their transitive dependencies, grouped
    /// into topological levels that may be executed concurrently. Each entry
    /// of a level is the task id together with the resolved dependency ids of
    /// that task within the run's closure.
    ///
    /// # Errors
    ///
    /// Returns an error if any of the given tasks is not in the graph.
    ///
    /// # Panics
    ///
    /// Panics if the graph is internally inconsistent.
    pub fn get_orders(&self, tasks: &[TaskId]) -> FyrerResult<Vec<Vec<(TaskId, Vec<TaskId>)>>> {
        for id in tasks {
            if !self.nodes.contains_key(id) {
                return Err(FyrerError::Graph(GraphError::TaskNotFound(id.to_string())));
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

        // Resolve each relevant task to the dependencies that are also relevant.
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

        // Compute the in-degree of every node in the relevant subgraph.
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::TaskGraph;
    use crate::{
        config::{RestartConfig, RestartStrategy},
        tasks::{Task, TaskId},
    };

    fn task(project: &str, name: &str, deps: Vec<String>) -> (TaskId, Task) {
        let id = TaskId::new(project, name);
        let task = Task {
            project_name: project.to_string(),
            project_root: "./".into(),
            env: HashMap::new(),
            task_name: name.to_string(),
            cmd: "echo hi".into(),
            depends_on: deps,
            inputs: vec![],
            outputs: vec![],
            ignore: vec![],
            cache: false,
            restart: RestartConfig {
                strategy: RestartStrategy::Never,
                delay: None,
            },
        };
        (id, task)
    }

    fn graph_with(entries: &[(&str, &str, Vec<String>)]) -> TaskGraph {
        let mut map = HashMap::new();
        for (project, name, deps) in entries {
            let (id, task) = task(project, name, deps.clone());
            map.insert(id, task);
        }
        TaskGraph::new(&map).unwrap()
    }

    #[test]
    fn get_orders_dedupes_shared_dependencies() {
        let graph = graph_with(&[
            ("web", "build", vec!["ui:build".into()]),
            ("web", "test", vec!["ui:build".into()]),
            ("ui", "build", vec![]),
        ]);
        graph.validate().unwrap();

        let order = graph
            .get_orders(&[TaskId::new("web", "build"), TaskId::new("web", "test")])
            .unwrap();

        assert_eq!(order.len(), 2);
        assert_eq!(order[0], vec![(TaskId::new("ui", "build"), vec![])]);
        let second: Vec<String> = order[1]
            .iter()
            .map(|(id, _)| id.to_string())
            .collect();
        assert!(second.contains(&"web:build".to_string()));
        assert!(second.contains(&"web:test".to_string()));
        for (_, deps) in &order[1] {
            assert_eq!(deps, &vec![TaskId::new("ui", "build")]);
        }
    }

    #[test]
    fn get_orders_rejects_unknown_tasks() {
        let graph = graph_with(&[("web", "build", vec![])]);
        assert!(graph.get_orders(&[TaskId::new("nope", "nope")]).is_err());
    }

    #[test]
    fn validate_rejects_cycles() {
        let graph = graph_with(&[
            ("web", "build", vec!["web:test".into()]),
            ("web", "test", vec!["web:build".into()]),
        ]);
        assert!(graph.validate().is_err());
    }

    #[test]
    fn validate_accepts_diamond_dependency() {
        let graph = graph_with(&[
            ("web", "build", vec!["ui:build".into()]),
            ("web", "test", vec!["ui:build".into()]),
            ("ui", "build", vec![]),
        ]);
        assert!(graph.validate().is_ok());
    }

    #[test]
    fn new_rejects_self_dependency() {
        let mut map = HashMap::new();
        let (id, task) = task("web", "build", vec!["web:build".into()]);
        map.insert(id, task);
        assert!(TaskGraph::new(&map).is_err());
    }
}
