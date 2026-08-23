use std::time::Duration;

use fyrer_core::{Attempt, ExecKey, TaskId, status::{TaskOutcome, TaskStatus, SkipReason}};

#[derive(Debug)]
pub enum SupCommand {
    Kill,
    Stdin(String),
}

#[derive(Debug, Clone)]
pub enum SupervisorMsg {
    Started { key: ExecKey, pid: u32 },
    Exited { key: ExecKey, outcome: TaskOutcome },
}

#[derive(Debug)]
pub enum EngineCommand {
    Start(RunPlan),
    Restart(Vec<TaskId>),
    Kill(Vec<TaskId>),
    /// A watched task's input files changed; restart it.
    FilesChanged(TaskId, Vec<std::path::PathBuf>),
    Shutdown,
}

#[derive(Debug, Clone)]
pub struct RunPlan {
    pub task_ids: Vec<TaskId>,
    pub concurrency: Option<usize>,
}

impl RunPlan {
    pub fn new(task_ids: Vec<TaskId>) -> Self {
        Self { task_ids, concurrency: None }
    }
    pub fn with_concurrency(mut self, n: usize) -> Self {
        self.concurrency = Some(n);
        self
    }
}

#[derive(Debug, Clone)]
pub enum EngineEvent {
    RunStarted { run: fyrer_core::RunId, planned: Vec<TaskId> },
    TaskReady(TaskId),
    TaskStarted { id: TaskId, attempt: Attempt, pid: u32 },
    TaskFinished { id: TaskId, outcome: TaskOutcome, final_status: TaskStatus },
    TaskLog { key: ExecKey, stream: LogStream, line: String },
    TaskCacheHit { id: TaskId },
    TaskSkipped { id: TaskId, reason: SkipReason },
    /// Watched input files changed for a task; a restart follows.
    FilesChanged { id: TaskId, paths: Vec<std::path::PathBuf> },
    /// A restart was requested for the task (watch or manual). Emitted at
    /// request time — the live attempt is being killed and will respawn.
    TaskRestarting { id: TaskId, killed_attempt: Attempt },
    DependentsStale { ids: Vec<TaskId> },
    /// All tasks reached a terminal state. Emitted immediately at completion,
    /// even if the engine keeps serving commands afterwards (interactive/TUI
    /// mode). May fire again after post-run restarts.
    RunCompleted(RunSummary),
    /// Final event before the engine task returns.
    RunFinished(RunSummary),
    NonFatalError { task_id: Option<TaskId>, error: String },
    Warning { task_id: Option<TaskId>, message: String },
}

#[derive(Debug, Clone, Copy)]
pub enum LogStream {
    Stdout,
    Stderr,
    System,
}

#[derive(Debug, Clone)]
pub struct RunSummary {
    pub total: usize,
    pub successful: usize,
    pub cached: usize,
    pub failed: usize,
    pub skipped: usize,
    pub duration: Duration,
}
