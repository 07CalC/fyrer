use crate::{
    error::{FyrerError, FyrerResult, state::StateError},
    graph::TaskGraph,
    logger::LogMessage,
    tasks::TaskMap,
};
use std::{collections::HashMap, sync::OnceLock};
use tokio::sync::mpsc::Sender;

#[derive(Debug)]
pub struct GlobalState {
    pub task_graph: TaskGraph,
    pub task_map: TaskMap,
    pub global_env: HashMap<String, String>,
    pub log_sender: Sender<LogMessage>,
}

pub static GLOBAL_STATE: OnceLock<GlobalState> = OnceLock::new();

pub fn init(
    task_graph: TaskGraph,
    task_map: TaskMap,
    global_env: HashMap<String, String>,
    log_sender: Sender<LogMessage>,
) -> FyrerResult<()> {
    GLOBAL_STATE
        .set(GlobalState {
            task_graph,
            task_map,
            global_env,
            log_sender,
        })
        .map_err(|_| FyrerError::State(StateError::AlreadyInitialized))
}

pub fn get() -> &'static GlobalState {
    GLOBAL_STATE.get().expect("Global state is not initialized")
}
