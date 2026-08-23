//! Supervisor actor: owns exactly one child process for exactly one attempt.
//!
//! The supervisor is the *only* code in the system that touches a `Child`.
//! It is created by the engine, receives [`SupCommand`]s over a dedicated
//! channel, streams logs to the router + event bus, and terminates by sending
//! [`SupervisorMsg::Exited`] and dropping the child.

use std::{
    sync::Arc,
    time::Instant,
};
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

use fyrer_core::{
    ExecKey,
    spec::TaskSpec,
    status::{ExitReason, TaskOutcome},
};
use fyrer_log::{LogLine, LogStream as RouterLogStream};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::{broadcast, mpsc},
};

use crate::events::{EngineEvent, LogStream, SupCommand, SupervisorMsg};

pub struct SupervisorOpts {
    pub key: ExecKey,
    pub spec: Arc<TaskSpec>,
}

/// Spawn the supervisor for one attempt.
///
/// - `cmd_rx` — control channel (Kill / Stdin), created by the engine
/// - `ev_tx`  — lifecycle events back to the engine (Started / Exited)
/// - `log_tx` — log lines to the LogRouter
/// - `events` — data-only broadcast mirror of log lines for reporters
pub fn spawn_supervisor(
    opts: SupervisorOpts,
    mut cmd_rx: mpsc::Receiver<SupCommand>,
    ev_tx: mpsc::UnboundedSender<SupervisorMsg>,
    log_tx: mpsc::Sender<LogLine>,
    events: broadcast::Sender<EngineEvent>,
) -> tokio::task::JoinHandle<TaskOutcome> {
    tokio::spawn(async move {
        let key = opts.key;
        let spec = opts.spec;
        let start = Instant::now();

        let mut command = fyrer_process::build_command(&spec);
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(e) => {
                // Spawn failure is a terminal outcome; nothing to supervise.
                let outcome =
                    TaskOutcome::new(key.attempt, ExitReason::SpawnError(e.to_string()), start.elapsed());
                let _ = ev_tx.send(SupervisorMsg::Exited {
                    key: key.clone(),
                    outcome: outcome.clone(),
                });
                return outcome;
            }
        };

        let _ = ev_tx.send(SupervisorMsg::Started {
            key: key.clone(),
            pid: child.id().unwrap_or(0),
        });

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let mut stdin = child.stdin.take();

        let log_pipers = pipe_logs(&key, stdout, stderr, log_tx, events);

        let deadline = spec.timeout.map(|d| Instant::now() + d);
        let mut timed_out = false;

        let outcome = loop {
            let remaining = deadline.map(|dl| dl.saturating_duration_since(Instant::now()));
            tokio::select! {
                status = child.wait() => {
                    let duration = start.elapsed();
                    // Drain pipers so trailing output isn't lost.
                    for piper in log_pipers {
                        let _ = piper.await;
                    }
                    break TaskOutcome::new(key.attempt, classify_status(status, timed_out), duration);
                }
                Some(cmd) = cmd_rx.recv() => match cmd {
                    SupCommand::Stdin(input) => {
                        if let Some(stdin) = stdin.as_mut() {
                            let _ = stdin.write_all(input.as_bytes()).await;
                            let _ = stdin.flush().await;
                        }
                    }
                    SupCommand::Kill => fyrer_process::kill_process_group(&mut child).await,
                },
                _ = async {
                    if let Some(remaining) = remaining {
                        tokio::time::sleep(remaining).await;
                    }
                }, if remaining.is_some() => {
                    timed_out = true;
                    fyrer_process::kill_process_group(&mut child).await;
                    // Reaping happens via `child.wait()` on the next loop pass.
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

/// Map an exit status to an [`ExitReason`], preferring signal info (unix) and
/// flagging timeouts so upstream policy can distinguish them from user kills.
fn classify_status(
    status: std::io::Result<std::process::ExitStatus>,
    timed_out: bool,
) -> ExitReason {
    match status {
        Ok(s) if s.success() => ExitReason::Success(s.code().unwrap_or(0)),
        Ok(s) => {
            #[cfg(unix)]
            if let Some(signal) = s.signal() {
                return ExitReason::Signal(signal);
            }
            if timed_out {
                ExitReason::Timeout
            } else {
                ExitReason::Failure(s.code().unwrap_or(-1))
            }
        }
        Err(e) => ExitReason::SpawnError(e.to_string()),
    }
}

/// Forward stdout/stderr line-by-line to the LogRouter and the reporter bus.
#[allow(clippy::type_complexity)]
fn pipe_logs(
    key: &ExecKey,
    stdout: Option<tokio::process::ChildStdout>,
    stderr: Option<tokio::process::ChildStderr>,
    log_tx: mpsc::Sender<LogLine>,
    events: broadcast::Sender<EngineEvent>,
) -> Vec<tokio::task::JoinHandle<()>> {
    let mut pipers = Vec::new();
    if let Some(out) = stdout {
        pipers.push(spawn_piper(key.clone(), out, RouterLogStream::Stdout, log_tx.clone(), events.clone()));
    }
    if let Some(err) = stderr {
        pipers.push(spawn_piper(key.clone(), err, RouterLogStream::Stderr, log_tx, events));
    }
    pipers
}

fn spawn_piper<R: tokio::io::AsyncRead + Unpin + Send + 'static>(
    key: ExecKey,
    stream: R,
    kind: RouterLogStream,
    log_tx: mpsc::Sender<LogLine>,
    events: broadcast::Sender<EngineEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut lines = BufReader::new(stream).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = log_tx
                .send(LogLine {
                    key: key.clone(),
                    stream: kind,
                    line: line.clone(),
                })
                .await;
            let event_kind = match kind {
                RouterLogStream::Stdout | RouterLogStream::System => LogStream::Stdout,
                RouterLogStream::Stderr => LogStream::Stderr,
            };
            let _ = events.send(EngineEvent::TaskLog {
                key: key.clone(),
                stream: event_kind,
                line,
            });
        }
    })
}
