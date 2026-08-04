use std::collections::{HashMap, HashSet};

use anyhow::Result;
use ratatui::widgets::ListState;
use tokio::sync::mpsc;

use crate::{
    TaskId,
    config::TaskMap,
    events::{AppEvent, TaskCommand},
    tasks::TaskStatus,
    tui::render,
};

pub async fn run_coordinator(
    task_ids: Vec<TaskId>,
    task_map: &TaskMap,
    mut event_rx: mpsc::Receiver<AppEvent>,
    event_tx: mpsc::Sender<AppEvent>,
) -> Result<()> {
    let mut terminal = ratatui::init();
    // All mutable runtime state — owned solely by this function
    let mut running: HashMap<TaskId, mpsc::Sender<TaskCommand>> = HashMap::new();
    let mut statuses: HashMap<TaskId, TaskStatus> = task_ids
        .iter()
        .map(|id| (id.clone(), TaskStatus::Waiting))
        .collect();
    let mut logs: HashMap<TaskId, Vec<String>> =
        task_ids.iter().map(|id| (id.clone(), Vec::new())).collect();
    let mut pending_restart: HashSet<TaskId> = HashSet::new();
    let mut list_state = ListState::default().with_selected(Some(0));
    let mut should_quit = false;
    loop {
        // ── Draw ──
        let selected_idx = list_state.selected().unwrap_or(0);
        let selected_task = task_ids.get(selected_idx).cloned();
        let selected_logs = selected_task
            .as_ref()
            .and_then(|id| logs.get(id))
            .map_or_else(Vec::new, Clone::clone);
        let snapshot: Vec<(String, TaskStatus)> = task_ids
            .iter()
            .map(|id| {
                let status = statuses.get(id).cloned().unwrap_or(TaskStatus::Waiting);
                (id.to_string(), status)
            })
            .collect();
        terminal.draw(|f| {
            render(f, &snapshot, &selected_logs, &mut list_state);
        })?;
        // ── Receive next event ──
        let event = match event_rx.recv().await {
            Some(e) => e,
            None => break, // all senders dropped → everything is done
        };
        // ── Handle ──
        match event {
            AppEvent::Stdout { task_id, line } => {
                logs.entry(task_id).or_default().push(line);
            }
            AppEvent::Stderr { task_id, line } => {
                logs.entry(task_id).or_default().push(format!("⚠ {line}"));
            }
            AppEvent::TaskSpawned {
                task_id,
                command_sender,
            } => {
                running.insert(task_id.clone(), command_sender);
                statuses.insert(task_id, TaskStatus::Running);
            }
            AppEvent::TaskComplete { task_id } => {
                running.remove(&task_id);
                if pending_restart.remove(&task_id) {
                    statuses.insert(task_id.clone(), TaskStatus::Restarting);
                    let task = &task_map[&task_id];
                    match task.spawn(event_tx.clone()) {
                        Ok(spawned) => {
                            running.insert(task_id.clone(), spawned.command_sender);
                            statuses.insert(task_id, TaskStatus::Running);
                        }
                        Err(e) => {
                            logs.entry(task_id.clone())
                                .or_default()
                                .push(format!("⚠ restart failed: {e}"));
                            statuses.insert(
                                task_id,
                                TaskStatus::Failed {
                                    code: -1,
                                    error: Some(e.to_string()),
                                },
                            );
                        }
                    }
                } else {
                    statuses.insert(task_id, TaskStatus::Complete);
                }
            }
            AppEvent::TaskFailed {
                task_id,
                exit_code: code,
                error,
            } => {
                running.remove(&task_id);
                if pending_restart.remove(&task_id) {
                    let task = &task_map[&task_id];
                    match task.spawn(event_tx.clone()) {
                        Ok(spawned) => {
                            running.insert(task_id.clone(), spawned.command_sender);
                            statuses.insert(task_id, TaskStatus::Running);
                        }
                        Err(e) => {
                            logs.entry(task_id.clone())
                                .or_default()
                                .push(format!("⚠ restart failed: {e}"));
                            statuses.insert(
                                task_id,
                                TaskStatus::Failed {
                                    code: -1,
                                    error: Some(e.to_string()),
                                },
                            );
                        }
                    }
                } else {
                    statuses.insert(task_id, TaskStatus::Failed { code, error });
                }
            }
            AppEvent::FileChanged { task_id } => {
                if pending_restart.contains(&task_id) {
                    continue;
                }
                if let Some(tx) = running.get(&task_id) {
                    let _ = tx.send(TaskCommand::Kill).await;
                    pending_restart.insert(task_id.clone());
                    statuses.insert(task_id, TaskStatus::Restarting);
                }
            }
            AppEvent::KeyPress(key) => {
                use crossterm::event::KeyCode;
                match key.code {
                    KeyCode::Char('q') => {
                        for (_, tx) in running.drain() {
                            let _ = tx.send(TaskCommand::Kill).await;
                        }
                        should_quit = true;
                    }
                    KeyCode::Char('j') | KeyCode::Down => list_state.select_next(),
                    KeyCode::Char('k') | KeyCode::Up => list_state.select_previous(),
                    _ => {}
                }
            }
            AppEvent::Tick => {} // re-render happens at top of loop
        }
        if should_quit {
            break;
        }
    }
    ratatui::restore();
    Ok(())
}
