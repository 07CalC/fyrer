use std::{
    os::unix::process::ExitStatusExt,
    path::PathBuf,
    process::Stdio,
    time::{Duration, Instant},
};

use anyhow::Result;
use glob::glob;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::broadcast::Sender,
    task::JoinHandle,
};

use crate::{
    cache::hash::{OutputDigest, hash_file, hash_kv},
    env::EnvMap,
    events::{AppEvent, TaskCommand},
    task::error::TaskError,
};

mod error;
mod graph;
mod id;
mod map;
mod process;
mod status;
pub(crate) use graph::TaskGraph;
pub(crate) use id::TaskId;
pub(crate) use map::TaskMap;
pub(crate) use process::ProcessResult;
pub(crate) use process::TaskProcess;
pub(crate) use process::TaskState;
pub(crate) use status::TaskStatus;

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
        let start_time = Instant::now();
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

        // main loop: owns the child process and waits for it to finish, sending events to
        // the event channel when it does, also listens for kill and stdin commands from
        // the command channel and acts on them, and kills the process if it exceeds the
        // configured timeout
        let task_id = self.id.clone();
        let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(1);
        let timeout = self.timeout;
        let handle = tokio::spawn(async move {
            let deadline = timeout.map(|duration| Instant::now() + duration);
            let mut timed_out = false;
            loop {
                let remaining =
                    deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()));
                let sleep = remaining.map(tokio::time::sleep);
                tokio::select! {
                    status = child.wait() => {
                        let duration = start_time.elapsed();
                        let _ = stdout_handle.await;
                        let _ = stderr_handle.await;
                        let timeout_error = timed_out
                            .then(|| "task exceeded its timeout and was killed".to_string());
                        let (event, process_result) = match &status {
                            Ok(s) if s.success() => (
                                AppEvent::TaskComplete {
                                    task_id: task_id.clone(),
                                },
                                ProcessResult::Success {
                                    exit_code: s.code().unwrap_or(0),
                                    duration,
                                },
                            ),
                            Ok(s) => {
                                if let Some(_) = s.signal() {
                                        (AppEvent::TaskComplete {
                                            task_id: task_id.clone(),
                                        },
                                        ProcessResult::Success { exit_code: s.code().unwrap_or(0), duration })
                                } else {
                                        (AppEvent::TaskFailed {
                                            task_id: task_id.clone(),
                                            exit_code: s.code().unwrap_or(-1),
                                            error: timeout_error.clone(),
                                        },
                                        ProcessResult::Failure {
                                            exit_code: s.code().unwrap_or(-1),
                                            duration,
                                            error: timeout_error,
                                        })
                                }
                            },
                            Err(e) => {
                                let error = format!(
                                    "Failed to wait for task {}: {}",
                                    task_id, e
                                );

                                (
                                    AppEvent::TaskFailed {
                                        task_id: task_id.clone(),
                                        exit_code: -1,
                                        error: Some(error.clone()),
                                    },
                                    ProcessResult::Failure {
                                        exit_code: -1,
                                        duration,
                                        error: Some(error),
                                    },
                                )
                            }
                        };
                        let _ = event_tx.send(event);
                        return process_result;
                    }
                    Some(cmd) = command_rx.recv() => {
                        match cmd {
                            TaskCommand::Stdin(input) => {
                                let _ = stdin.write_all(input.as_bytes()).await;
                                let _ = stdin.flush().await;
                            }
                            TaskCommand::Kill => {
                                Self::terminate(&mut child).await;
                                // not waiting for the child to exit here, it will be handled in the
                                // next iteration of the loop when the child exits and the status is
                                // received
                            }
                        }
                    }
                    _ = async {
                        if let Some(sleep) = sleep {
                            sleep.await;
                        }
                    }, if remaining.is_some() => {
                        timed_out = true;
                        Self::terminate(&mut child).await;
                        // reaping happens via child.wait() on the next iteration of the loop
                    }
                }
            }
        });
        return Ok(TaskProcess {
            handle,
            command_tx,
            task_id: self.id.clone(),
        });
    }

    async fn terminate(child: &mut tokio::process::Child) {
        // sending SIGKILL to the process group ensures every descendant of the task is
        // terminated, not just the immediate child process
        #[cfg(unix)]
        if let Some(pid) = child.id() {
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
        }
        let _ = child.kill().await;
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
                let _ = stdout_tx.send(AppEvent::TaskLog {
                    task_id: stdout_id.clone(),
                    stream: crate::events::LogStream::Stdout,
                    line,
                });
            }
        });
        let stderr_handle = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = stderr_tx.send(AppEvent::TaskLog {
                    task_id: stderr_id.clone(),
                    stream: crate::events::LogStream::Stderr,
                    line,
                });
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

    pub fn cache_key(&self, task_map: TaskMap) -> Result<String> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(self.id.to_string().as_bytes());
        hasher.update(self.cmd.as_bytes());
        hasher.update(self.cwd.to_string_lossy().as_bytes());

        let mut env: Vec<_> = self.env.iter().collect();
        env.sort_by_key(|(key, _)| *key);
        for (key, value) in env {
            hash_kv(&mut hasher, key, value);
        }

        //TODO: implement ignore behaviour
        for input in &self.inputs {
            let entries = match glob(input) {
                Ok(p) => p,
                Err(_) => continue,
            };
            for entry in entries {
                if let Ok(path) = entry {
                    if path.is_file() {
                        hash_file(&mut hasher, &path)?;
                    }
                }
            }
        }
        for dep_id in &self.depends_on {
            if let Some(dep_task) = task_map.get(dep_id) {
                hasher.update(dep_task.cache_key(task_map.clone())?.as_bytes());
            }
        }
        Ok(hasher.finalize().to_hex().to_string())
    }

    pub fn output_digest(&self) -> Result<OutputDigest> {
        let mut hasher = blake3::Hasher::new();
        for output in &self.outputs {
            let entries = match glob(output) {
                Ok(p) => p,
                Err(_) => continue,
            };
            for entry in entries {
                if let Ok(path) = entry {
                    if path.is_file() {
                        hash_file(&mut hasher, &path)?;
                    }
                }
            }
        }
        Ok(hasher.finalize().to_hex().to_string())
    }

    pub fn resolve_outputs(&self) -> Vec<PathBuf> {
        let mut resolved = Vec::new();
        for output in &self.outputs {
            let entries = match glob(self.cwd.join(output).to_string_lossy().as_ref()) {
                Ok(p) => p,
                Err(_) => continue,
            };
            for entry in entries {
                if let Ok(path) = entry {
                    resolved.push(path);
                }
            }
        }
        resolved
    }
}
