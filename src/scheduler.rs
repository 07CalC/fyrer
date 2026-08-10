use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::{
    Task, TaskId,
    cache::{
        CacheMetadata, CacheProvider, CacheStatus,
        hash::{get_hash, hash_output_files},
    },
    config::TaskMap,
    events::AppEvent,
};

pub fn initialize_pipeline(event_bus_sender: &tokio::sync::mpsc::Sender<AppEvent>) {
    spawn_input_collector(event_bus_sender.clone());
    spawn_ctrl_c_handler(event_bus_sender.clone());
}
pub async fn schedule(
    levels: Vec<Vec<(TaskId, Vec<TaskId>)>>,
    task_map: Arc<TaskMap>,
    cache_provider: Arc<dyn CacheProvider>,
    event_bus_sender: tokio::sync::mpsc::Sender<AppEvent>,
) {
    let mut failed: HashSet<TaskId> = HashSet::new();

    for batch in levels {
        let mut handles: Vec<(
            TaskId,
            String,
            std::time::Instant,
            tokio::task::JoinHandle<bool>,
        )> = Vec::with_capacity(batch.len());

        for (task_id, deps) in batch {
            if deps.iter().any(|dep| failed.contains(dep)) {
                failed.insert(task_id.clone());
                let _ = event_bus_sender
                    .send(AppEvent::TaskFailed {
                        task_id: task_id.clone(),
                        exit_code: -1,
                        error: Some(format!("skipped: a dependency of {task_id} failed")),
                    })
                    .await;
                continue;
            }

            let task = &task_map[&task_id];

            if task.cache {
                match get_hash(task, &task_map) {
                    Err(e) => {
                        failed.insert(task_id.clone());
                        let _ = event_bus_sender
                            .send(AppEvent::TaskFailed {
                                task_id: task_id.clone(),
                                exit_code: -1,
                                error: Some(format!("cache: failed to compute hash: {e}")),
                            })
                            .await;
                    }
                    Ok(hash) if cache_provider.contains(&hash) => {
                        let output_hash = hash_output_files(&task).unwrap_or("".to_string());
                        match cache_provider.restore(&hash, &output_hash) {
                            Ok(true) => {
                                let _ = event_bus_sender
                                    .send(AppEvent::Stdout {
                                        task_id: task_id.clone(),
                                        line: format!(
                                            "cache hit [{hash:.12}] — skipping execution"
                                        ),
                                    })
                                    .await;
                                let _ = event_bus_sender
                                    .send(AppEvent::TaskCacheHit {
                                        task_id: task_id.clone(),
                                    })
                                    .await;
                                continue;
                            }
                            Ok(false) => {
                                let _ = event_bus_sender
                                    .send(AppEvent::Stderr {
                                        task_id: task_id.clone(),
                                        line: "cache: restore returned false, re-running task"
                                            .to_string(),
                                    })
                                    .await;
                            }
                            Err(e) => {
                                let _ = event_bus_sender
                                    .send(AppEvent::Stderr {
                                        task_id: task_id.clone(),
                                        line: format!(
                                            "cache: restore failed ({e}), re-running task"
                                        ),
                                    })
                                    .await;
                            }
                        }
                        spawn_task(task, task_id, hash, &event_bus_sender, &mut handles).await;
                    }
                    Ok(hash) => {
                        spawn_task(task, task_id, hash, &event_bus_sender, &mut handles).await;
                    }
                }
            } else {
                spawn_task(
                    task,
                    task_id,
                    String::new(),
                    &event_bus_sender,
                    &mut handles,
                )
                .await;
            }
        }

        for (task_id, hash, started_at, handle) in handles {
            let success = handle.await.unwrap_or(false);
            if !success {
                failed.insert(task_id.clone());
                continue;
            }

            if hash.is_empty() {
                continue;
            }

            let task = &task_map[&task_id];
            let duration_ms = started_at.elapsed().as_millis() as u64;
            let output_paths = resolve_output_dirs(task);
            let output_hash = hash_output_files(task).unwrap_or("".to_string());

            let metadata = CacheMetadata {
                task: task_id.to_string(),
                hash: hash.clone(),
                cmd: task.cmd.clone(),
                dependencies: task.depends_on.clone(),
                duration_ms,
                exit_code: 0,
                cache: CacheStatus::Miss,
                cache_key: Some(hash.clone()),
                timestamp: unix_timestamp_secs(),
                output_hash: output_hash.clone(),
            };

            match cache_provider.save(&hash, &output_paths, metadata) {
                Ok(_) => {
                    let _ = event_bus_sender
                        .send(AppEvent::Stdout {
                            task_id: task_id.clone(),
                            line: format!("cache saved [{hash:.12}]"),
                        })
                        .await;
                }
                Err(e) => {
                    // Non-fatal: the task succeeded; just warn about the save.
                    let _ = event_bus_sender
                        .send(AppEvent::Stderr {
                            task_id: task_id.clone(),
                            line: format!("cache: failed to save ({e})"),
                        })
                        .await;
                }
            }
        }
    }
    let _ = event_bus_sender.send(AppEvent::RunFinished).await;
}
async fn spawn_task(
    task: &Task,
    task_id: TaskId,
    hash: String,
    event_bus_sender: &tokio::sync::mpsc::Sender<AppEvent>,
    handles: &mut Vec<(
        TaskId,
        String,
        std::time::Instant,
        tokio::task::JoinHandle<bool>,
    )>,
) {
    match task.spawn(event_bus_sender.clone()) {
        Ok(spawned_task) => {
            let _ = event_bus_sender
                .send(AppEvent::TaskSpawned {
                    task_id: task_id.clone(),
                    command_sender: spawned_task.command_sender,
                })
                .await;
            handles.push((
                task_id,
                hash,
                std::time::Instant::now(),
                spawned_task.handle,
            ));
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

fn resolve_output_dirs(task: &Task) -> Vec<PathBuf> {
    task.outputs
        .iter()
        .map(|pattern| {
            let root = pattern
                .split(['*', '?', '['])
                .next()
                .unwrap_or(pattern)
                .trim_end_matches('/');
            task.project_root.join(Path::new(root))
        })
        .collect()
}

fn unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
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
                    match result {
                        Ok(Some(crossterm::event::Event::Key(key_event))) => {
                            if event_bus_sender.send(AppEvent::KeyPress(key_event)).await.is_err() {
                                break;
                            }
                        }
                        Ok(Some(crossterm::event::Event::Mouse(mouse_event))) => {
                            let dir = match mouse_event.kind {
                                crossterm::event::MouseEventKind::ScrollUp => {
                                    Some(crate::events::ScrollDirection::Up)
                                }
                                crossterm::event::MouseEventKind::ScrollDown => {
                                    Some(crate::events::ScrollDirection::Down)
                                }
                                _ => None,
                            };
                            if let Some(d) = dir
                                && event_bus_sender.send(AppEvent::MouseScroll(d)).await.is_err()
                            {
                                break;
                            }
                        }
                        _ => {}
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
