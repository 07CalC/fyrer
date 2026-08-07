use std::{sync::Arc, time::Duration};

use crate::{TaskId, config::TaskMap, events::AppEvent};

/// Spawns the background tasks that feed the event bus: a periodic ticker and
/// keyboard input collector, plus a Ctrl-C handler. These must be started once,
/// before the orchestrator event loop begins.
pub fn initialize_pipeline(event_bus_sender: &tokio::sync::mpsc::Sender<AppEvent>) {
    spawn_input_collector(event_bus_sender.clone());
    spawn_ctrl_c_handler(event_bus_sender.clone());
}

/// Spawns every task across the dependency levels, waiting for each level to
/// finish before starting the next.
pub async fn schedule(
    levels: Vec<Vec<TaskId>>,
    task_map: Arc<TaskMap>,
    event_bus_sender: tokio::sync::mpsc::Sender<AppEvent>,
) {
    for batch in levels {
        let mut handles = Vec::with_capacity(batch.len());
        for task_id in batch {
            let task = &task_map[&task_id];
            match task.spawn(event_bus_sender.clone()) {
                Ok(spawned_task) => {
                    let _ = event_bus_sender
                        .send(AppEvent::TaskSpawned {
                            task_id: task_id.clone(),
                            command_sender: spawned_task.command_sender,
                        })
                        .await;
                    handles.push(spawned_task.handle);
                }
                Err(e) => {
                    let _ = event_bus_sender
                        .send(AppEvent::TaskFailed {
                            task_id: task_id.clone(),
                            exit_code: -1,
                            error: Some(format!("Failed to spawn task {task_id}: {e}")),
                        })
                        .await;
                }
            }
        }
        futures::future::join_all(handles).await;
    }
}

pub fn spawn_input_collector(event_bus_sender: tokio::sync::mpsc::Sender<AppEvent>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if event_bus_sender.send(AppEvent::Tick).await.is_err() {
                        break;
                    }
                }
                result = tokio::task::spawn_blocking(|| {
                        if crossterm::event::poll(Duration::from_millis(50)).unwrap_or(false) {
                            crossterm::event::read().ok()
                        } else {
                            None
                        }
                    }) => {
                    if let Ok(Some(crossterm::event::Event::Key(key_event))) = result
                        && event_bus_sender.send(AppEvent::KeyPress(key_event)).await.is_err()
                    {
                        break;
                    }
                }
            }
        }
    });
}

pub fn spawn_ctrl_c_handler(event_bus_sender: tokio::sync::mpsc::Sender<AppEvent>) {
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        let _ = event_bus_sender
            .send(AppEvent::KeyPress(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('q'),
                crossterm::event::KeyModifiers::NONE,
            )))
            .await;
    });
}
