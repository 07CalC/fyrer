use std::{sync::Arc, time::{Duration, Instant}};

use fyrer_core::{ExecKey, spec::TaskSpec, status::{ExitReason, TaskOutcome}};
use fyrer_log::{LogLine, LogStream as RouterStream};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::mpsc,
};

use crate::events::{EngineEvent, LogStream, SupervisorMsg, SupCommand};

pub struct SupervisorOpts {
    pub key: ExecKey,
    pub spec: Arc<TaskSpec>,
}

/// Spawn a supervisor for one attempt. Returns JoinHandle that resolves to outcome.
/// The supervisor owns the Child exclusively. It sends logs to `log_tx` and lifecycle
/// events to `ev_tx`. Control commands arrive via `cmd_rx`.
pub fn spawn_supervisor(
    opts: SupervisorOpts,
    mut cmd_rx: mpsc::Receiver<SupCommand>,
    ev_tx: mpsc::UnboundedSender<SupervisorMsg>,
    log_tx: mpsc::Sender<LogLine>,
    event_broadcast: tokio::sync::broadcast::Sender<EngineEvent>,
) -> tokio::task::JoinHandle<TaskOutcome> {
    tokio::spawn(async move {
        let key = opts.key.clone();
        let spec = opts.spec;
        let start = Instant::now();

        // Build command
        let mut command = fyrer_process::build_command(&spec);
        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(e) => {
                let outcome = TaskOutcome::new(
                    key.attempt,
                    ExitReason::SpawnError(e.to_string()),
                    start.elapsed(),
                );
                let _ = ev_tx.send(SupervisorMsg::Exited {
                    key: key.clone(),
                    outcome: outcome.clone(),
                });
                return outcome;
            }
        };

        let pid = child.id().unwrap_or(0);
        let _ = ev_tx.send(SupervisorMsg::Started {
            key: key.clone(),
            pid,
        });

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let mut stdin = child.stdin.take();

        // spawn log piping
        let mut log_handles = Vec::new();
        if let Some(out) = stdout {
            let k = key.clone();
            let tx = log_tx.clone();
            let bcast = event_broadcast.clone();
            log_handles.push(tokio::spawn(async move {
                let mut lines = BufReader::new(out).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx
                        .send(LogLine {
                            key: k.clone(),
                            stream: RouterStream::Stdout,
                            line: line.clone(),
                        })
                        .await;
                    let _ = bcast.send(EngineEvent::TaskLog {
                        key: k.clone(),
                        stream: LogStream::Stdout,
                        line,
                    });
                }
            }));
        }
        if let Some(err) = stderr {
            let k = key.clone();
            let tx = log_tx.clone();
            let bcast = event_broadcast.clone();
            log_handles.push(tokio::spawn(async move {
                let mut lines = BufReader::new(err).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx
                        .send(LogLine {
                            key: k.clone(),
                            stream: RouterStream::Stderr,
                            line: line.clone(),
                        })
                        .await;
                    let _ = bcast.send(EngineEvent::TaskLog {
                        key: k.clone(),
                        stream: LogStream::Stderr,
                        line,
                    });
                }
            }));
        }

        let deadline = spec.timeout.map(|d| Instant::now() + d);
        let mut timed_out = false;

        let outcome = loop {
            let remaining = deadline.map(|dl| dl.saturating_duration_since(Instant::now()));
            let sleep = remaining.map(tokio::time::sleep);

            tokio::select! {
                status = child.wait() => {
                    let duration = start.elapsed();
                    for h in log_handles { let _ = h.await; }
                    let exit = match status {
                        Ok(s) if s.success() => ExitReason::Success(s.code().unwrap_or(0)),
                        Ok(s) => {
                            #[cfg(unix)]
                            {
                                use std::os::unix::process::ExitStatusExt;
                                if let Some(sig) = s.signal() {
                                    ExitReason::Signal(sig)
                                } else if timed_out {
                                    ExitReason::Timeout
                                } else {
                                    ExitReason::Failure(s.code().unwrap_or(-1))
                                }
                            }
                            #[cfg(windows)]
                            {
                                if timed_out { ExitReason::Timeout } else { ExitReason::Failure(s.code().unwrap_or(-1)) }
                            }
                        },
                        Err(e) => ExitReason::SpawnError(e.to_string()),
                    };
                    break TaskOutcome::new(key.attempt, exit, duration);
                }
                Some(cmd) = cmd_rx.recv() => {
                    match cmd {
                        SupCommand::Stdin(s) => {
                            if let Some(stdin) = stdin.as_mut() {
                                let _ = stdin.write_all(s.as_bytes()).await;
                                let _ = stdin.flush().await;
                            }
                        }
                        SupCommand::Kill => {
                            fyrer_process::kill_process_group(&mut child).await;
                        }
                    }
                }
                _ = async {
                    if let Some(s) = sleep { s.await; }
                }, if remaining.is_some() => {
                    timed_out = true;
                    fyrer_process::kill_process_group(&mut child).await;
                }
            }
        };

        let _ = ev_tx.send(SupervisorMsg::Exited {
            key: key.clone(),
            outcome: outcome.clone(),
        });
        outcome
    })
}
