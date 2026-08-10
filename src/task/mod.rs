use std::{path::PathBuf, process::Stdio, time::Duration};

use anyhow::Result;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::mpsc::{Sender, channel},
    task::JoinHandle,
};

use crate::{
    env::EnvMap,
    events::{AppEvent, TaskCommand},
    task::{error::TaskError, process::TaskProcess},
};

mod error;
mod graph;
mod id;
mod map;
mod process;
pub(crate) use id::TaskId;

#[derive(Debug, Clone)]
pub struct Task {
    pub id: TaskId,
    pub env: EnvMap,
    pub cache: bool,
    pub watch: bool,
    pub persistent: bool,
    pub timeout: Option<Duration>,
    pub cwd: PathBuf,
    pub cmd: String,
    pub depends_on: Vec<TaskId>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub ignore: Vec<String>,
}

impl Task {
    pub fn new(
        id: TaskId,
        env: EnvMap,
        cache: bool,
        watch: bool,
        persistent: bool,
        timeout: Option<Duration>,
        cwd: PathBuf,
        cmd: String,
        depends_on: Vec<TaskId>,
        inputs: Vec<String>,
        outputs: Vec<String>,
        ignore: Vec<String>,
    ) -> Self {
        Self {
            id,
            env,
            cache,
            watch,
            persistent,
            timeout,
            cwd,
            cmd,
            depends_on,
            inputs,
            outputs,
            ignore,
        }
    }

    pub fn spawn(&self, event_tx: Sender<AppEvent>) -> Result<TaskProcess> {
        let mut command = self.command();
        let mut child = command.spawn().map_err(|e| TaskError::TaskSpawnFailed {
            task_id: self.id.clone(),
            error: e.to_string(),
        })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| TaskError::FailedToTakeStdio {
                task_id: self.id.clone(),
                stdio: "stdout".to_string(),
            })?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| TaskError::FailedToTakeStdio {
                task_id: self.id.clone(),
                stdio: "stderr".to_string(),
            })?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| TaskError::FailedToTakeStdio {
                task_id: self.id.clone(),
                stdio: "stdin".to_string(),
            })?;

        let task_id = self.id.clone();
        let event_tx_clone = event_tx.clone();
        // pipe logs from stdout and stderr to the event channel
        let (stdout_handle, stderr_handle) =
            Self::pipe_logs(task_id, stdout, stderr, event_tx_clone);

        // main lopp: owns the child process and waits for it to finish, sending events to
        // the event channel when it does, also listens for kill and stdin commands from
        // the command channel and acts on them
        let task_id = self.id.clone();
        let (command_tx, mut command_rx) = channel(1);
        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    status = child.wait() => {
                        let success = matches!(status, Ok(exit_status) if exit_status.success());
                        let _ = stdout_handle.await;
                        let _ = stderr_handle.await;
                        let event = match status {
                            Ok(s) if s.success() => AppEvent::TaskComplete{
                                task_id: task_id.clone(),
                            },
                            Ok(s) => AppEvent::TaskFailed{
                                task_id: task_id.clone(),
                                exit_code: s.code().unwrap_or(-1),
                                error: None,
                            },
                            Err(e) => AppEvent::TaskFailed{
                                task_id: task_id.clone(),
                                exit_code: -1,
                                error: Some(format!("Failed to wait for task {}: {}", task_id, e)),
                            },
                        };
                        let _ = event_tx.send(event).await;
                        return success;
                    }
                    Some(cmd) = command_rx.recv() => {
                        match cmd {
                            TaskCommand::Stdin(input) => {
                                let _ = stdin.write_all(input.as_bytes()).await;
                                let _ = stdin.flush().await;
                            }
                            TaskCommand::Kill => {
                                #[cfg(unix)]
                                if let Some(pid) = child.id() {
                                    unsafe {
                                        libc::kill(-(pid as i32), libc::SIGKILL);
                                    }
                                }
                                let _ = child.kill().await;
                                // not waiting for the child to exit here, it will be handled in the
                                // next iteration of the loop when the child exits and the status is
                                // received

                            }
                        }
                    }

                }
            }
        });
        return Ok(TaskProcess { handle, command_tx });
    }

    fn pipe_logs(
        task_id: TaskId,
        stdout: tokio::process::ChildStdout,
        stderr: tokio::process::ChildStderr,
        event_tx: Sender<AppEvent>,
    ) -> (JoinHandle<()>, JoinHandle<()>) {
        let stdout_tx = event_tx.clone();
        let stderr_tx = event_tx.clone();
        let stdout_id = task_id.clone();
        let stderr_id = task_id.clone();
        let stdout_handle = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = stdout_tx
                    .send(AppEvent::TaskLog {
                        task_id: stdout_id.clone(),
                        stream: crate::events::LogStream::Stdout,
                        line,
                    })
                    .await;
            }
        });
        let stderr_handle = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = stderr_tx
                    .send(AppEvent::TaskLog {
                        task_id: stderr_id.clone(),
                        stream: crate::events::LogStream::Stderr,
                        line,
                    })
                    .await;
            }
        });
        (stdout_handle, stderr_handle)
    }

    fn command(&self) -> Command {
        #[cfg(unix)]
        let mut command = Command::new("sh");
        #[cfg(windows)]
        let mut command = Command::new("cmd");

        #[cfg(unix)]
        command.arg("-c").arg(&self.cmd);
        #[cfg(windows)]
        command.arg("/C").arg(&self.cmd);

        command.current_dir(&self.cwd);
        command.envs(&self.env);
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        command.stdin(Stdio::piped());
        // setting the pgid to 0 will make the child process the leader of a new process
        // this is important because we want to be able to kill the entire process group
        // when the task is stopped
        command.process_group(0);
        command
    }
}
