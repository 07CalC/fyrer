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
    /// Sent once the scheduler has finished every level, including all
    /// post-completion work (e.g. cache saves). Auto-quit waits for it so
    /// final messages are always shown.
    RunFinished,
    Tick,
}

#[derive(Debug)]
pub enum TaskCommand {
    Stdin(String),
    Kill,
}
