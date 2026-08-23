use std::time::Duration;

use crate::id::Attempt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitReason {
    Success(i32),
    Failure(i32),
    Signal(i32),
    Timeout,
    Killed,
    SpawnError(String),
}

impl ExitReason {
    pub fn is_success(&self) -> bool {
        matches!(self, ExitReason::Success(0)) || matches!(self, ExitReason::Success(_)) && {
            if let ExitReason::Success(c) = self { *c == 0 } else { false }
        }
    }
    pub fn code(&self) -> i32 {
        match self {
            ExitReason::Success(c) | ExitReason::Failure(c) => *c,
            ExitReason::Signal(s) => 128 + s,
            ExitReason::Timeout => 124,
            ExitReason::Killed => 137,
            ExitReason::SpawnError(_) => -1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    UpstreamFailed,
    UpstreamSkipped,
}

#[derive(Debug, Clone)]
pub struct TaskOutcome {
    pub attempt: Attempt,
    pub exit: ExitReason,
    pub duration: Duration,
    pub exit_code: i32,
}

impl TaskOutcome {
    pub fn new(attempt: Attempt, exit: ExitReason, duration: Duration) -> Self {
        let exit_code = exit.code();
        Self { attempt, exit, duration, exit_code }
    }
    pub fn is_success(&self) -> bool {
        matches!(self.exit, ExitReason::Success(0))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Ready,
    Running { attempt: Attempt },
    Succeeded { attempt: Attempt },
    Failed { attempt: Attempt },
    Cached { attempt: Attempt },
    Skipped { reason: SkipReason },
    Restarting { from: Attempt },
    Stale,
}

impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskStatus::Succeeded { .. }
                | TaskStatus::Failed { .. }
                | TaskStatus::Cached { .. }
                | TaskStatus::Skipped { .. }
        )
    }
    pub fn is_running(&self) -> bool {
        matches!(self, TaskStatus::Running { .. } | TaskStatus::Restarting { .. })
    }
}
