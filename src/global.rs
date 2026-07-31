use std::sync::OnceLock;

use crate::{graph::TaskGraph, tasks::TaskMap};

#[derive(Debug)]
pub struct GlobalState {
    pub task_graph: TaskGraph,
    pub task_map: TaskMap,
}

pub static GLOBAL_STATE: OnceLock<GlobalState> = OnceLock::new();

pub fn init(task_graph: TaskGraph, task_map: TaskMap) {
    GLOBAL_STATE
        .set(GlobalState {
            task_graph,
            task_map,
        })
        .expect("Global state has already been initialized");
}

pub fn get() -> &'static GlobalState {
    GLOBAL_STATE.get().expect("Global state is not initialized")
}
