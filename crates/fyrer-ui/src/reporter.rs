use anyhow::Result;
use tokio::{
    sync::{broadcast, mpsc},
    task::JoinHandle,
};

use fyrer_engine::events::{EngineCommand, EngineEvent};

pub trait Reporter: Send + 'static {
    fn start(
        self,
        rx: broadcast::Receiver<EngineEvent>,
    ) -> JoinHandle<Result<()>>;

    /// Optional control channel for restart/kill. Pass `None` for read-only reporting.
    fn start_with_control(
        self,
        rx: broadcast::Receiver<EngineEvent>,
        _cmd_tx: Option<mpsc::Sender<EngineCommand>>,
    ) -> JoinHandle<Result<()>>
    where
        Self: Sized,
    {
        self.start(rx)
    }
}
