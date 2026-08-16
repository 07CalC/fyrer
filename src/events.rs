use crossterm::event::KeyEvent;
use tokio::sync::mpsc::Sender;

use crate::{executor::scheduler::RunSummary, task::TaskId};

#[derive(Debug, Clone, Copy)]
pub enum ScrollDirection {
    Up,
    Down,
}

#[derive(Debug, Clone, Copy)]
pub enum LogStream {
    Stdout,
    Stderr,
    System,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    TaskLog {
        task_id: TaskId,
        stream: LogStream,
        line: String,
    },
    TaskComplete {
        task_id: TaskId,
    },
    TaskFailed {
        task_id: TaskId,
        exit_code: i32,
        error: Option<String>,
    },
    TaskCacheHit {
        task_id: TaskId,
    },
    TaskSkipped {
        task_id: TaskId,
    },
    RestartRequest {
        task_id: TaskId,
    },
    FileChanged {
        task_id: TaskId,
    },
    KeyPress(KeyEvent),
    MouseScroll(ScrollDirection),
    TaskSpawned {
        task_id: TaskId,
        command_tx: Sender<TaskCommand>,
    },
    RunFinished(RunSummary),
    NonFatalError {
        task_id: Option<TaskId>,
        error: String,
    },
    Warning {
        task_id: Option<TaskId>,
        message: String,
    },
    Shutdown,
    Tick,
}

#[derive(Debug)]
pub enum TaskCommand {
    Stdin(String),
    Kill,
}
