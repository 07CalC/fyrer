use thiserror::Error;

use crate::task::id::TaskId;

#[derive(Debug, Error)]
pub enum TaskError {
    #[error("Task {task_id} failed to spawn: {error}")]
    TaskSpawnFailed { task_id: TaskId, error: String },
    #[error("")]
    FailedToTakeStdio { task_id: TaskId, stdio: String },
}

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("Task {dependent} has a missing dependency: {dependency}")]
    MissingDependency {
        dependent: TaskId,
        dependency: TaskId,
    },
    #[error("Task {task_id} has a self-dependency")]
    SelfDependency { task_id: TaskId },
    #[error("Task graph has a cycle involving task {task_id}")]
    CycleDetected { task_id: TaskId },
    #[error("Task {task_id} not found in the graph")]
    TaskNotFound { task_id: TaskId },
}
