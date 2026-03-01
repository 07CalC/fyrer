use crate::cli::logger::{Log, LogKind};
use crate::config::Service;
use crate::kill_process::kill_process;
use crate::spawn_service::spawn_service;
use globset::GlobSetBuilder;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::fs;
use std::path::Path;
use tokio::process::Child;
use tokio::sync::mpsc::Sender;
use tokio::{
    sync::mpsc,
    time::{Duration, sleep},
};

pub async fn run_with_watch(service: Service, color: colored::Color, logger: Sender<Log>) {
    let (tx, mut rx) = mpsc::channel(1);
    let watch_dir = Path::new(&service.dir);

    let abs_service_dir =
        fs::canonicalize(&service.dir).unwrap_or_else(|_| Path::new(&service.dir).to_path_buf());

    let ignore = service.clone().ignore.unwrap_or_else(Vec::new);

    let mut builder = GlobSetBuilder::new();
    for pattern in &ignore {
        if let Ok(glob) = globset::Glob::new(pattern) {
            builder.add(glob);
        } else {
            let _ = logger
                .send(Log {
                    service: service.name.clone(),
                    message: format!("Invalid ignore pattern: {}", pattern),
                    kind: LogKind::Error,
                    color,
                })
                .await;
        }
    }

    let glob_set = builder.build().unwrap();

    let _watcher = {
        let logger = logger.clone();
        let service_name = service.name.clone();

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| match res {
                Ok(event) => {
                    if !event.paths.is_empty() {
                        let _ = tx.try_send(event.paths.clone());
                    }
                }
                Err(e) => {
                    let _ = logger.try_send(Log {
                        service: service_name.clone(),
                        message: format!("Watch error: {:?}", e),
                        kind: LogKind::Error,
                        color,
                    });
                }
            },
            Config::default(),
        )
        .unwrap_or_else(|e| {
            eprintln!("Failed to create watcher: {}", e);
            std::process::exit(1);
        });

        watcher
            .watch(watch_dir, RecursiveMode::Recursive)
            .unwrap_or_else(|e| {
                eprintln!("Failed to watch {}: {}", service.dir, e);
                std::process::exit(1);
            });

        watcher
    };

    let _ = logger
        .send(Log {
            service: service.name.clone(),
            message: format!("Watching {} for changes...", service.dir),
            kind: LogKind::System,
            color,
        })
        .await;

    let mut child: Option<Child> = spawn_service(&service, color, false, logger.clone()).await;

    loop {
        tokio::select! {

            changed_files = rx.recv() => {
                if let Some(paths) = changed_files {

                    let filtered_paths: Vec<_> = paths.into_iter()
                        .filter(|path| {
                            let rel_path = path
                                .strip_prefix(&abs_service_dir)
                                .unwrap_or(path);
                            !glob_set.is_match(rel_path)
                        })
                        .collect();

                    if filtered_paths.is_empty() {
                        continue;
                    }

                    if let Some(mut c) = child.take() {
                        let _ = logger.send(Log {
                            service: service.name.clone(),
                            message: "File changed, restarting service...".into(),
                            kind: LogKind::System,
                            color,
                        }).await;
                        kill_process(&mut c).await;
                        sleep(Duration::from_millis(500)).await;
                    }

                    child = spawn_service(
                        &service,
                        color,
                        false,
                        logger.clone()
                    ).await;

                    sleep(Duration::from_millis(500)).await;

                    while rx.try_recv().is_ok() {}
                }
            }

            _ = async {
                if let Some(ref mut c) = child {
                    c.wait().await
                } else {
                    std::future::pending().await
                }
            } => {

                let _ = logger.send(Log {
                    service: service.name.clone(),
                    message: "Service exited, restarting...".into(),
                    kind: LogKind::System,
                    color,
                }).await;

                sleep(Duration::from_millis(1000)).await;

                child = spawn_service(
                    &service,
                    color,
                    false,
                    logger.clone()
                ).await;
            }
        }
    }
}
