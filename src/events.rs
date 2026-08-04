use crossterm::event::KeyEvent;

use crate::TaskId;

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
    FileChanged {
        task_id: TaskId,
    },
    KeyPress(KeyEvent),
    Tick,
}

#[derive(Debug)]
pub enum TaskCommand {
    Stdin(String),
    Kill,
}
