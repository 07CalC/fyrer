use crossterm::event::KeyEvent;
use tokio::sync::mpsc::Sender;

use crate::TaskId;

/// Direction of a mouse scroll event.
#[derive(Debug, Clone, Copy)]
pub enum ScrollDirection {
    Up,
    Down,
}

/// Which output stream a log line came from.
#[derive(Debug, Clone, Copy)]
pub enum LogStream {
    Stdout,
    Stderr,
}

#[derive(Debug)]
pub enum AppEvent {
    Stdout {
        task_id: TaskId,
        line: String,
    },
    Stderr {
        task_id: TaskId,
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
    /// Emitted by the scheduler when a task is skipped due to a cache hit.
    TaskCacheHit {
        task_id: TaskId,
    },
    FileChanged {
        task_id: TaskId,
    },
    KeyPress(KeyEvent),
    MouseScroll(ScrollDirection),
    TaskSpawned {
        task_id: TaskId,
        command_sender: Sender<TaskCommand>,
    },
    Tick,
}

#[derive(Debug)]
pub enum TaskCommand {
    Stdin(String),
    Kill,
}
