use thiserror::Error;

use crate::task::id::TaskId;

#[derive(Debug, Error)]
pub enum TaskError {
    #[error("Task {task_id} failed to spawn: {error}")]
    TaskSpawnFailed { task_id: TaskId, error: String },
    #[error("")]
    FailedToTakeStdio { task_id: TaskId, stdio: String },
}
