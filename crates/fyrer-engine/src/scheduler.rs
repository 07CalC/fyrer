use std::collections::{HashMap, HashSet, VecDeque};

use fyrer_core::{TaskId, TaskGraph};

/// Ready-queue policy. Pure logic — no IO.
pub struct SchedulerState {
    pub relevant: HashSet<TaskId>,
    pub pending_deps: HashMap<TaskId, usize>,
    pub ready: VecDeque<TaskId>,
    pub graph: TaskGraph,
}

impl SchedulerState {
    pub fn new(graph: TaskGraph, roots: &[TaskId]) -> Self {
        let relevant = graph.transitive_closure(roots);
        let pending_deps = graph.in_degree_map(&relevant);
        let mut ready = VecDeque::new();
        for (id, deg) in &pending_deps {
            if *deg == 0 {
                ready.push_back(id.clone());
            }
        }
        Self {
            relevant,
            pending_deps,
            ready,
            graph,
        }
    }

    pub fn pop_ready(&mut self) -> Option<TaskId> {
        self.ready.pop_front()
    }
    pub fn push_ready(&mut self, id: TaskId) {
        if !self.ready.contains(&id) {
            self.ready.push_back(id);
        }
    }

    /// Call when `id` completed successfully. Returns newly-ready tasks.
    pub fn on_success(&mut self, id: &TaskId) -> Vec<TaskId> {
        let mut newly = Vec::new();
        for dep in self.graph.dependents_of(id) {
            if !self.relevant.contains(&dep) {
                continue;
            }
            if let Some(cnt) = self.pending_deps.get_mut(&dep) {
                if *cnt > 0 {
                    *cnt -= 1;
                    if *cnt == 0 {
                        self.ready.push_back(dep.clone());
                        newly.push(dep.clone());
                    }
                }
            }
        }
        newly
    }

    /// For failure cascade, find all transitive dependents that should be skipped.
    pub fn transitive_dependents_to_skip(&self, failed: &TaskId) -> Vec<TaskId> {
        let mut out = Vec::new();
        let mut stack = vec![failed.clone()];
        let mut seen = HashSet::new();
        seen.insert(failed.clone());
        while let Some(cur) = stack.pop() {
            for dep in self.graph.dependents_of(&cur) {
                if !self.relevant.contains(&dep) {
                    continue;
                }
                if seen.insert(dep.clone()) {
                    out.push(dep.clone());
                    stack.push(dep.clone());
                }
            }
        }
        out
    }

    pub fn is_relevant(&self, id: &TaskId) -> bool {
        self.relevant.contains(id)
    }
}
