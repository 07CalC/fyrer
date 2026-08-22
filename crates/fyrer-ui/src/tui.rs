use anyhow::Result;
use tokio::{sync::broadcast, task::JoinHandle};

use fyrer_engine::events::EngineEvent;

use crate::reporter::Reporter;

pub struct Tui;

impl Tui {
    pub fn new() -> Self { Self }
}

impl Reporter for Tui {
    fn start(self, rx: broadcast::Receiver<EngineEvent>) -> JoinHandle<Result<()>> {
        // Delegates to plain reporter for now. A full ratatui port that handles
        // EngineEvent (TaskStarted/TaskLog/TaskFinished/CacheHit/Skipped/Restarting)
        // and sends EngineCommand::Restart on 'r' is tracked for the next iteration.
        // This keeps TUI mode functional (shows logs) while the interactive
        // restart affordance is wired through EngineHandle.
        crate::plain::PlainReporter::default().start(rx)
    }
}
