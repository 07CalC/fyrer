use anyhow::Result;
use tokio::{sync::broadcast, task::JoinHandle};

use fyrer_engine::events::EngineEvent;

pub trait Reporter: Send + 'static {
    fn start(
        self,
        rx: broadcast::Receiver<EngineEvent>,
    ) -> JoinHandle<Result<()>>;
}
