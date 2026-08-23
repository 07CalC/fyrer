use std::sync::Arc;

use anyhow::Result;
use fyrer_cache::provider::CacheProvider;
use fyrer_core::{TaskId, spec::TaskRegistry, TaskGraph};
use fyrer_log::LogRouter;
use tokio::sync::{broadcast, mpsc};

use crate::{
    engine::Engine,
    events::{EngineCommand, EngineEvent, RunPlan, RunSummary},
};

pub struct EngineHandle {
    cmd_tx: mpsc::Sender<EngineCommand>,
    event_tx: broadcast::Sender<EngineEvent>,
    join: tokio::task::JoinHandle<Result<RunSummary>>,
}

impl EngineHandle {
    pub async fn restart(&self, ids: Vec<TaskId>) -> Result<()> {
        self.cmd_tx
            .send(EngineCommand::Restart(ids))
            .await
            .map_err(|_| anyhow::anyhow!("engine closed"))?;
        Ok(())
    }
    pub async fn kill(&self, ids: Vec<TaskId>) -> Result<()> {
        self.cmd_tx
            .send(EngineCommand::Kill(ids))
            .await
            .map_err(|_| anyhow::anyhow!("engine closed"))?;
        Ok(())
    }
    pub async fn shutdown(&self) -> Result<()> {
        let _ = self.cmd_tx.send(EngineCommand::Shutdown).await;
        Ok(())
    }
    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.event_tx.subscribe()
    }
    pub fn cmd_sender(&self) -> mpsc::Sender<EngineCommand> {
        self.cmd_tx.clone()
    }
    /// Alias for cmd_sender, used by watcher integration
    pub fn subscribe_cmd_tx(&self) -> mpsc::Sender<EngineCommand> {
        self.cmd_tx.clone()
    }
    pub async fn wait(self) -> Result<RunSummary> {
        self.join.await?
    }
}

pub struct EngineBuilder {
    registry: TaskRegistry,
    graph: TaskGraph,
    cache: Arc<dyn CacheProvider>,
    log_router: Arc<LogRouter>,
    concurrency: Option<usize>,
    interactive: bool,
}

impl EngineBuilder {
    pub fn new(registry: TaskRegistry, graph: TaskGraph, cache: Arc<dyn CacheProvider>) -> Self {
        let log_router = Arc::new(LogRouter::new(500, None));
        Self {
            registry,
            graph,
            cache,
            log_router,
            concurrency: None,
            interactive: true,
        }
    }
    pub fn concurrency(mut self, n: usize) -> Self {
        self.concurrency = Some(n);
        self
    }
    pub fn log_router(mut self, r: Arc<LogRouter>) -> Self {
        self.log_router = r;
        self
    }
    /// Interactive engines keep serving restart/kill commands after the run
    /// completes (TUI browsing, watch mode). Non-interactive ones exit as soon
    /// as every task reaches a terminal state.
    pub fn interactive(mut self, v: bool) -> Self {
        self.interactive = v;
        self
    }
    pub fn build(self) -> Engine {
        let (event_tx, _) = broadcast::channel(1024);
        Engine::new(
            self.registry,
            self.graph,
            self.cache,
            self.log_router,
            event_tx,
            self.concurrency,
        )
    }

    pub fn spawn(self, plan: RunPlan) -> EngineHandle {
        let (event_tx, _) = broadcast::channel(1024);
        let engine = Engine::new(
            self.registry,
            self.graph,
            self.cache,
            self.log_router,
            event_tx.clone(),
            self.concurrency,
        );
        let (cmd_tx, cmd_rx) = mpsc::channel(32);
        let interactive = self.interactive;
        let join =
            tokio::spawn(async move { engine.run_with_receiver_inner(plan, cmd_rx, interactive).await });
        EngineHandle {
            cmd_tx,
            event_tx,
            join,
        }
    }
}
