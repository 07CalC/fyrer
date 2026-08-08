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
        cache::get_hash,
    },
    config::TaskMap,
    events::AppEvent,
};

/// Spawns the background tasks that feed the event bus: a periodic ticker and
/// keyboard input collector, plus a Ctrl-C handler. These must be started once,
/// before the orchestrator event loop begins.
pub fn initialize_pipeline(event_bus_sender: &tokio::sync::mpsc::Sender<AppEvent>) {
    spawn_input_collector(event_bus_sender.clone());
    spawn_ctrl_c_handler(event_bus_sender.clone());
}

/// Spawns every task across the dependency levels, waiting for each level to
/// finish before starting the next. Tasks whose dependencies have already
/// failed are skipped (cascading the failure to their own dependents).
///
/// Before spawning a task the scheduler checks the cache: if a valid entry
/// exists it restores the outputs and emits [`AppEvent::TaskCacheHit`] instead
/// of actually running the task. After each successful run the outputs are
/// saved so subsequent runs hit the cache.
pub async fn schedule(
    levels: Vec<Vec<(TaskId, Vec<TaskId>)>>,
    task_map: Arc<TaskMap>,
    cache_provider: Arc<dyn CacheProvider>,
    event_bus_sender: tokio::sync::mpsc::Sender<AppEvent>,
) {
    let mut failed: HashSet<TaskId> = HashSet::new();

    for batch in levels {
        // Each entry: (task_id, cache_hash_or_empty, start_instant, join_handle)
        let mut handles: Vec<(TaskId, String, std::time::Instant, tokio::task::JoinHandle<bool>)> =
            Vec::with_capacity(batch.len());

        for (task_id, deps) in batch {
            // Cascade failures from dependencies.
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

            // ── Cache check (only for tasks that opt in) ──────────────────
            if task.cache {
                match get_hash(task, &task_map) {
                    Err(e) => {
                        // Hash computation failed — log and fall through to run.
                        let _ = event_bus_sender
                            .send(AppEvent::Stderr {
                                task_id: task_id.clone(),
                                line: format!("cache: failed to compute hash: {e}"),
                            })
                            .await;
                    }
                    Ok(hash) if cache_provider.contains(&hash) => {
                        // Cache hit — try to restore.
                        match cache_provider.restore(&hash) {
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
                                continue; // skip spawning
                            }
                            Ok(false) => {
                                // Entry found but not restorable — fall through.
                                let _ = event_bus_sender
                                    .send(AppEvent::Stderr {
                                        task_id: task_id.clone(),
                                        line: "cache: restore returned false, re-running task"
                                            .to_string(),
                                    })
                                    .await;
                            }
                            Err(e) => {
                                // Restore error — log as warning and fall through.
                                let _ = event_bus_sender
                                    .send(AppEvent::Stderr {
                                        task_id: task_id.clone(),
                                        line: format!("cache: restore failed ({e}), re-running task"),
                                    })
                                    .await;
                            }
                        }
                        // Fall through: run the task normally
                        spawn_task(
                            task,
                            task_id,
                            hash,
                            &event_bus_sender,
                            &mut handles,
                        )
                        .await;
                    }
                    Ok(hash) => {
                        // Cache miss — run the task.
                        spawn_task(
                            task,
                            task_id,
                            hash,
                            &event_bus_sender,
                            &mut handles,
                        )
                        .await;
                    }
                }
            } else {
                // Caching disabled for this task — run unconditionally.
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

        // ── Wait for the entire batch and save caches on success ──────────
        for (task_id, hash, started_at, handle) in handles {
            let success = handle.await.unwrap_or(false);
            if !success {
                failed.insert(task_id.clone());
                continue;
            }

            // Only attempt to save if the task opted into caching and we have
            // a valid hash (non-empty string means hash was computed).
            if hash.is_empty() {
                continue;
            }

            let task = &task_map[&task_id];
            let duration_ms = started_at.elapsed().as_millis() as u64;
            let output_paths = resolve_output_dirs(task);

            let metadata = CacheMetadata {
                task: task_id.to_string(),
                hash: hash.clone(),
                cmd: task.cmd.clone(),
                dependencies: task.depends_on.clone(),
                duration_ms,
                exit_code: 0,
                outputs: task.outputs.clone(),
                cache: CacheStatus::Miss,
                cache_key: Some(hash.clone()),
                timestamp: unix_timestamp_secs(),
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
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Spawns a single task, sends [`AppEvent::TaskSpawned`], and records its
/// handle in `handles` for the batch-await loop.
async fn spawn_task(
    task: &Task,
    task_id: TaskId,
    hash: String,
    event_bus_sender: &tokio::sync::mpsc::Sender<AppEvent>,
    handles: &mut Vec<(TaskId, String, std::time::Instant, tokio::task::JoinHandle<bool>)>,
) {
    match task.spawn(event_bus_sender.clone()) {
        Ok(spawned_task) => {
            let _ = event_bus_sender
                .send(AppEvent::TaskSpawned {
                    task_id: task_id.clone(),
                    command_sender: spawned_task.command_sender,
                })
                .await;
            handles.push((task_id, hash, std::time::Instant::now(), spawned_task.handle));
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

/// Resolves the "root" directories/files to archive from a task's output
/// patterns. Takes the non-glob prefix of each pattern so that whole
/// directories are archived (e.g. `dist/**/*` → `dist/`).
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

/// Returns the current time as seconds since the Unix epoch.
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
