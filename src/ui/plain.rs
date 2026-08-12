use anyhow::Result;
use tokio::{sync::broadcast::Receiver, task::JoinHandle};

use crate::events::{AppEvent, LogStream};

use super::Ui;

#[derive(Default)]
pub struct PlainUi;

impl Ui for PlainUi {
    fn start(self, mut rx: Receiver<AppEvent>) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(AppEvent::TaskLog {
                        task_id,
                        line,
                        stream,
                    }) => match stream {
                        LogStream::Stdout => println!("[{task_id}] {line}"),
                        LogStream::Stderr => eprintln!("[{task_id}] \x1b[31m⚠ {line}\x1b[0m"),
                        LogStream::System => {}
                    },

                    Ok(AppEvent::TaskComplete { task_id }) => {
                        println!("\x1b[32m✓\x1b[0m [{task_id}] complete");
                    }
                    Ok(AppEvent::TaskFailed {
                        task_id,
                        exit_code,
                        error,
                    }) => {
                        let extra = error
                            .as_deref()
                            .map(|e| format!(": {e}"))
                            .unwrap_or_default();
                        eprintln!("\x1b[31m✗\x1b[0m [{task_id}] failed (exit {exit_code}){extra}");
                    }
                    Ok(AppEvent::TaskCacheHit { task_id }) => {
                        println!("\x1b[35m⚡\x1b[0m [{task_id}] cache hit");
                    }
                    Ok(AppEvent::TaskSkipped { task_id }) => {
                        println!("\x1b[90m○\x1b[0m [{task_id}] skipped");
                    }

                    Ok(AppEvent::RunFinished(summary)) => {
                        break;
                    }

                    Ok(AppEvent::Shutdown) => break,

                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        eprintln!(
                            "\x1b[33m⚠\x1b[0m [fyrer] {n} log lines were dropped (channel lagged)"
                        );
                    }

                    Ok(_) => {}
                }
            }
            Ok(())
        })
    }
}
