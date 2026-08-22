use anyhow::Result;
use tokio::{sync::broadcast, task::JoinHandle};

use fyrer_engine::events::EngineEvent;

use crate::reporter::Reporter;

#[derive(Default)]
pub struct PlainReporter;

impl Reporter for PlainReporter {
    fn start(self, mut rx: broadcast::Receiver<EngineEvent>) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(EngineEvent::TaskLog { key, line, stream }) => {
                        match stream {
                            fyrer_engine::events::LogStream::Stdout => {
                                println!("[{}] {}", key.task, line)
                            }
                            fyrer_engine::events::LogStream::Stderr => {
                                eprintln!("[{}] \x1b[31m{}\x1b[0m", key.task, line)
                            }
                            fyrer_engine::events::LogStream::System => {}
                        }
                    }
                    Ok(EngineEvent::TaskStarted { id, attempt, .. }) => {
                        println!("[{}#{}] started", id, attempt.0);
                    }
                    Ok(EngineEvent::TaskFinished { id, outcome, .. }) => {
                        if outcome.is_success() {
                            println!("\x1b[32m✓\x1b[0m [{}] complete", id);
                        } else {
                            eprintln!("\x1b[31m✗\x1b[0m [{}] failed ({:?})", id, outcome.exit);
                        }
                    }
                    Ok(EngineEvent::TaskCacheHit { id }) => {
                        println!("\x1b[35m⚡\x1b[0m [{}] cache hit", id);
                    }
                    Ok(EngineEvent::TaskSkipped { id, .. }) => {
                        println!("\x1b[90m○\x1b[0m [{}] skipped", id);
                    }
                    Ok(EngineEvent::RunFinished(_)) => break,
                    Ok(_) => {}
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        eprintln!("\x1b[33m⚠\x1b[0m [fyrer] {n} events lagged");
                    }
                }
            }
            Ok(())
        })
    }
}
