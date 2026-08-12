use anyhow::Result;
use tokio::{sync::broadcast::Receiver, task::JoinHandle};

use crate::events::AppEvent;

pub mod plain;
pub mod tui;

/// A UI backend that reacts to events broadcast by the orchestrator.
///
/// Implementors run their own event loop (usually on a dedicated thread) by
/// consuming [`AppEvent`]s from the supplied broadcast receiver. The handle
/// returned by [`Ui::start`] resolves once the UI has fully shut down — i.e.
/// the user has acknowledged the run summary and requested to quit.
pub trait Ui: Send + 'static {
    fn start(self, rx: Receiver<AppEvent>) -> JoinHandle<Result<()>>;
}
