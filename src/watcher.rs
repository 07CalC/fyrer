use crate::config::Project;
use crate::kill_process::kill_process;
use crate::spawn_service::spawn_service;
use colored::Colorize;
use globset::GlobSetBuilder;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::fs;
use std::path::Path;
use tokio::process::Child;
use tokio::{
    sync::mpsc,
    time::{Duration, sleep},
};

pub async fn run_with_watch(service: Project, color: colored::Color, max_name_len: usize) {
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
            eprintln!("Invalid ignore pattern: {}", pattern);
        }
    }
    let glob_set = builder.build().unwrap();

    let _watcher = {
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| match res {
                Ok(event) => {
                    if !event.paths.is_empty() {
                        let _ = tx.try_send(event.paths.clone());
                    }
                }
                Err(e) => {
                    eprintln!("Watch error: {:?}", e);
                }
            },
            Config::default(),
        )
        .unwrap_or_else(|e| {
            eprintln!("Failed to create file watcher: {}", e);
            std::process::exit(1);
        });

        watcher
            .watch(watch_dir, RecursiveMode::Recursive)
            .unwrap_or_else(|e| {
                eprintln!("Failed to watch directory {}: {}", service.dir, e);
                std::process::exit(1);
            });
        watcher
    };

    let out_prefix = format!("[{}]", service.name);
    let padded_name = format!("{:<width$}", out_prefix.clone(), width = max_name_len);

    println!(
        "├─{} ➤ Watching {} for changes...",
        padded_name.color(color).bold(),
        service.dir
    );

    let mut child: Option<Child> = spawn_service(&service, color, false, max_name_len).await;

    loop {
        tokio::select! {
            changed_files = rx.recv() => {
                if let Some(paths) = changed_files {
                    let filtered_paths: Vec<_> = paths.into_iter()
                        .filter(|path| {
                            let rel_path = path.strip_prefix(&abs_service_dir).unwrap_or(path);
                            !glob_set.is_match(rel_path)
                        })
                        .collect();

                    if filtered_paths.is_empty() {
                        continue;
                    }

                    if let Some(mut c) = child.take() {
                        let padded_name = format!("{:<width$}", out_prefix.clone(), width = max_name_len);
                        println!(
                            "├─{} ➤ File changed, restarting service...",
                            padded_name.color(color).bold()
                        );
                        kill_process(&mut c).await;
                        sleep(Duration::from_millis(500)).await;
                    }

                    child = spawn_service(&service, color, false, max_name_len).await;
                    sleep(Duration::from_millis(500)).await;

                    // drain remaining events
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
                let padded_name = format!("{:<width$}", out_prefix.clone(), width = max_name_len);
                println!(
                    "├─{} ➤ Service exited, restarting...",
                    padded_name.color(color).bold()
                );
                sleep(Duration::from_millis(1000)).await;
                child = spawn_service(&service, color, false, max_name_len).await;
            }
        }
    }
}
