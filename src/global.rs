use std::{collections::HashMap, hash::Hash, sync::OnceLock};

use tokio::sync::mpsc::Sender;

use crate::{
    graph::TaskGraph,
    logger::{LogMessage, Logger},
    tasks::TaskMap,
};

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
) {
    GLOBAL_STATE
        .set(GlobalState {
            task_graph,
            task_map,
            global_env,
            log_sender,
        })
        .expect("Global state has already been initialized");
}

pub fn get() -> &'static GlobalState {
    GLOBAL_STATE.get().expect("Global state is not initialized")
}
