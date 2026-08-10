use std::time::Duration;

use crate::{events::TaskCommand, task::TaskId};
use tokio::{sync::mpsc::Sender, task::JoinHandle};

#[derive(Debug)]

pub struct TaskProcess {
    pub task_id: TaskId,
    pub handle: JoinHandle<ProcessResult>,
    pub command_tx: Sender<TaskCommand>,
}

#[derive(Debug, Clone)]
pub enum ProcessResult {
    Success {
        exit_code: i32,
        duration: Duration,
    },
    Failure {
        exit_code: i32,
        duration: Duration,
        error: Option<String>,
    },
}
