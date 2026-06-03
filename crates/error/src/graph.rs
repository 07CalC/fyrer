use thiserror::Error;

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("cycle detected involving task '{0}'")]
    CycleDetected(String),
    #[error("task '{dependency}' referenced by '{dependent}' not found")]
    MissingDependency {
        dependent: String,
        dependency: String,
    },
    #[error("task '{0}' depends on itself")]
    SelfDependency(String),
    #[error("invalid dependency format '{dependency}' in task '{task}', expected 'project:task'")]
    InvalidTaskId { dependency: String, task: String },
}

