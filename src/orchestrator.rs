use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;

use crate::{
    TaskId,
    config::TaskMap,
    events::{AppEvent, LogStream, TaskCommand},
    scheduler::{initialize_pipeline, schedule},
    tasks::TaskStatus,
    tui::Ui,
};

/// Owns all mutable run state and drives the event loop, delegating rendering
/// to a [`Ui`] backend. The orchestrator is fully decoupled from the user
/// interface; which backend is used is decided by whoever constructs it.
pub struct Orchestrator {
    task_ids: Vec<TaskId>,
    task_map: Arc<TaskMap>,
    running: HashMap<TaskId, mpsc::Sender<TaskCommand>>,
    statuses: HashMap<TaskId, TaskStatus>,
    pending_restart: HashSet<TaskId>,
    total_tasks: usize,
    finished: usize,
    should_quit: bool,
    auto_quit: bool,
    ui: Box<dyn Ui>,
}

impl Orchestrator {
    #[must_use]
    pub fn new(task_ids: Vec<TaskId>, task_map: Arc<TaskMap>, ui: Box<dyn Ui>) -> Self {
        let statuses = task_ids
            .iter()
            .map(|id| (id.clone(), TaskStatus::Waiting))
            .collect();
        let total_tasks = task_ids.len();
        Self {
            task_ids,
            task_map,
            running: HashMap::new(),
            statuses,
            pending_restart: HashSet::new(),
            total_tasks,
            finished: 0,
            should_quit: false,
            auto_quit: false,
            ui,
        }
    }

    /// Sets whether the run exits once every task has finished. Interactive
    /// UIs typically keep running until the user quits; non-interactive ones
    /// finish as soon as the work is done.
    #[must_use]
    pub fn with_auto_quit(mut self, auto_quit: bool) -> Self {
        self.auto_quit = auto_quit;
        self
    }

    /// Runs the event loop until a quit condition is met.
    ///
    /// # Errors
    ///
    /// Returns an error if the UI backend fails to render.
    pub async fn run(
        &mut self,
        mut event_rx: mpsc::Receiver<AppEvent>,
        event_tx: mpsc::Sender<AppEvent>,
    ) -> Result<()> {
        loop {
            self.ui.render(&self.snapshot())?;
            let Some(event) = event_rx.recv().await else {
                break;
            };
            self.handle_event(event, &event_tx).await;
            if self.should_quit {
                break;
            }
        }
        self.ui.shutdown()?;
        Ok(())
    }

    fn snapshot(&self) -> Vec<(TaskId, TaskStatus)> {
        self.task_ids
            .iter()
            .map(|id| {
                let status = self
                    .statuses
                    .get(id)
                    .cloned()
                    .unwrap_or(TaskStatus::Waiting);
                (id.clone(), status)
            })
            .collect()
    }

    async fn handle_event(&mut self, event: AppEvent, event_tx: &mpsc::Sender<AppEvent>) {
        match event {
            AppEvent::Stdout { task_id, line } => {
                self.ui.push_log(&task_id, line, LogStream::Stdout);
            }
            AppEvent::Stderr { task_id, line } => {
                self.ui.push_log(&task_id, line, LogStream::Stderr);
            }
            AppEvent::TaskSpawned {
                task_id,
                command_sender,
            } => {
                self.running.insert(task_id.clone(), command_sender);
                self.statuses.insert(task_id, TaskStatus::Running);
            }
            AppEvent::TaskComplete { task_id } => {
                self.running.remove(&task_id);
                if self.pending_restart.remove(&task_id) {
                    self.restart(&task_id, event_tx);
                } else {
                    self.statuses.insert(task_id.clone(), TaskStatus::Complete);
                    self.mark_finished();
                }
            }
            AppEvent::TaskFailed {
                task_id,
                exit_code,
                error,
            } => {
                self.running.remove(&task_id);
                if self.pending_restart.remove(&task_id) {
                    self.restart(&task_id, event_tx);
                } else {
                    self.statuses.insert(
                        task_id,
                        TaskStatus::Failed {
                            code: exit_code,
                            error,
                        },
                    );
                    self.mark_finished();
                }
            }
            AppEvent::FileChanged { task_id } => {
                if self.pending_restart.contains(&task_id) {
                    return;
                }
                if let Some(tx) = self.running.get(&task_id) {
                    let _ = tx.send(TaskCommand::Kill).await;
                    self.pending_restart.insert(task_id.clone());
                    self.statuses.insert(task_id, TaskStatus::Restarting);
                }
            }
            AppEvent::KeyPress(key) => {
                use crossterm::event::KeyCode;
                match key.code {
                    KeyCode::Char('q') => {
                        for (_, tx) in self.running.drain() {
                            let _ = tx.send(TaskCommand::Kill).await;
                        }
                        while !self.running.is_empty() {}
                        self.should_quit = true;
                    }
                    KeyCode::Char('j') | KeyCode::Down => self.ui.navigate_next(),
                    KeyCode::Char('k') | KeyCode::Up => self.ui.navigate_previous(),
                    KeyCode::Char('u') => self.ui.scroll_logs_up_by(3),
                    KeyCode::Char('d') => self.ui.scroll_logs_down_by(3),
                    _ => {}
                }
            }
            AppEvent::MouseScroll(direction) => {
                use crate::events::ScrollDirection;
                match direction {
                    ScrollDirection::Up => self.ui.scroll_logs_up_by(3),
                    ScrollDirection::Down => self.ui.scroll_logs_down_by(3),
                }
            }
            AppEvent::Tick => {}
        }
    }

    fn restart(&mut self, task_id: &TaskId, event_tx: &mpsc::Sender<AppEvent>) {
        let task = &self.task_map[task_id];
        match task.spawn(event_tx.clone()) {
            Ok(spawned) => {
                self.running.insert(task_id.clone(), spawned.command_sender);
                self.statuses.insert(task_id.clone(), TaskStatus::Running);
            }
            Err(e) => {
                self.ui
                    .push_log(task_id, format!("restart failed: {e}"), LogStream::Stderr);
                self.statuses.insert(
                    task_id.clone(),
                    TaskStatus::Failed {
                        code: -1,
                        error: Some(e.to_string()),
                    },
                );
                self.mark_finished();
            }
        }
    }

    fn mark_finished(&mut self) {
        self.finished += 1;
        if self.auto_quit && self.finished >= self.total_tasks {
            self.should_quit = true;
        }
    }
}

/// Convenience entry point: builds the event pipeline and runs the
/// orchestrator with the given UI backend over the computed task levels.
///
/// # Errors
///
/// Returns an error if the UI backend fails to render or shut down.
pub async fn run(
    levels: Vec<Vec<TaskId>>,
    task_map: Arc<TaskMap>,
    ui: Box<dyn Ui>,
    auto_quit: bool,
) -> Result<()> {
    let (event_tx, event_rx) = mpsc::channel(task_map.len() * 10);
    let all_task_ids: Vec<TaskId> = levels.iter().flatten().cloned().collect();

    initialize_pipeline(&event_tx);
    let scheduler_tx = event_tx.clone();
    let scheduler_task_map = task_map.clone();
    tokio::spawn(async move { schedule(levels, scheduler_task_map, scheduler_tx).await });

    let mut orchestrator = Orchestrator::new(all_task_ids, task_map, ui).with_auto_quit(auto_quit);
    orchestrator.run(event_rx, event_tx).await
}
