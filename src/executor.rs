//! Process execution: spawning tasks, streaming output, and managing
//! long-running processes.

use std::process::Stdio;
use std::time::Duration;

use tokio::{
    io::{AsyncBufReadExt, AsyncRead},
    process::{Child, Command},
    sync::mpsc,
    task::JoinHandle,
};

use crate::{
    config::RestartStrategy,
    error::{FyrerError, FyrerResult, LoggerError, TaskError},
    global,
    logger::{LogMessage, LogType},
    tasks::{Task, TaskId},
};

/// A task that keeps running (e.g. a dev server) together with its process.
pub struct RunningTask {
    pub task: Task,
    /// The running child process.
    pub process: TaskProcess,
}

/// A spawned child process and the tasks streaming its output.
pub struct TaskProcess {
    /// The id of the task this process belongs to.
    pub task_id: TaskId,
    child: Child,
    stdout_reader: Option<JoinHandle<FyrerResult<()>>>,
    stderr_reader: Option<JoinHandle<FyrerResult<()>>>,
}

impl TaskProcess {
    /// Gracefully stops the process, escalating to `SIGKILL` if it does not
    /// exit within two seconds.
    pub async fn stop(&mut self) {
        self.signal_group(libc::SIGTERM);
        if tokio::time::timeout(Duration::from_secs(2), self.child.wait())
            .await
            .is_err()
        {
            self.signal_group(libc::SIGKILL);
            let _ = self.child.wait().await;
        }
        global::unregister_pid(&self.task_id);
    }

    /// Sends a signal to the entire process group of the child.
    #[cfg(unix)]
    fn signal_group(&self, signal: libc::c_int) {
        let Some(pid) = self.child.id() else {
            return;
        };
        // SAFETY: `-pid` refers to the child's process group, created with
        // `process_group(0)` in `start_task`. The pid is owned by `self.child`.
        unsafe {
            libc::kill(-pid.cast_signed(), signal);
        }
    }
}

impl Drop for TaskProcess {
    fn drop(&mut self) {
        self.signal_group(libc::SIGKILL);
        global::unregister_pid(&self.task_id);
    }
}

/// Runs the given tasks in topological order, spawning each batch
/// concurrently, and returns the long-running (watched) tasks.
///
/// # Errors
///
/// Returns an error if the task graph is malformed, a task is missing from
/// the task map, or any task in the batch fails.
pub async fn execute_tasks(tasks: &[TaskId]) -> FyrerResult<Vec<RunningTask>> {
    let state = global::get();
    let order = state.task_graph.get_orders(tasks)?;
    let mut running = Vec::new();

    for batch in order {
        let mut handles = Vec::new();
        for task_id in batch {
            let task = state
                .task_map
                .get(&task_id)
                .cloned()
                .ok_or_else(|| FyrerError::Task(TaskError::NotFound(task_id.to_string())))?;
            let handle = match task.restart.strategy {
                RestartStrategy::FileChange => {
                    tokio::spawn(async move { start_task(task).await.map(Some) })
                }
                RestartStrategy::OnFailure | RestartStrategy::Never => {
                    tokio::spawn(async move { execute_task(task).await.map(|()| None) })
                }
            };
            handles.push((task_id, handle));
        }

        for (task_id, handle) in handles {
            if let Some(process) = join(handle, &task_id).await? {
                let task =
                    state.task_map.get(&task_id).cloned().ok_or_else(|| {
                        FyrerError::Task(TaskError::NotFound(task_id.to_string()))
                    })?;
                running.push(RunningTask { task, process });
            }
        }
    }

    Ok(running)
}

/// Runs a task to completion, returning an error if it fails.
///
/// # Errors
///
/// Returns an error if the task cannot be spawned, exits with a non-zero
/// status, or its output cannot be read.
pub async fn execute_task(task: Task) -> FyrerResult<()> {
    let mut process = start_task(task).await?;
    let task_id = process.task_id.clone();
    let status = process.child.wait().await.map_err(|source| {
        FyrerError::Task(TaskError::Wait {
            task: task_id.to_string(),
            source,
        })
    })?;
    join_reader(process.stdout_reader.take(), &task_id).await?;
    join_reader(process.stderr_reader.take(), &task_id).await?;

    if !status.success() {
        return Err(FyrerError::Task(TaskError::Failed {
            task: task_id.to_string(),
            code: status.code().unwrap_or(-1),
        }));
    }
    Ok(())
}

/// Spawns a task's command and wires its stdout/stderr to the logger.
///
/// # Errors
///
/// Returns an error if the command cannot be spawned, its stdout or stderr
/// cannot be captured, or a shutdown was requested while spawning.
pub async fn start_task(task: Task) -> FyrerResult<TaskProcess> {
    let task_id = TaskId::new(&task.project_name, &task.task_name);
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(&task.cmd)
        .current_dir(&task.project_root)
        .envs(&task.env)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    command.process_group(0);

    let mut child = command.spawn().map_err(|source| {
        FyrerError::Task(TaskError::Spawn {
            task: task_id.to_string(),
            source,
        })
    })?;

    if global::is_shutting_down() {
        if let Some(pid) = child.id() {
            #[cfg(unix)]
            // SAFETY: `-pid` targets the process group we just created.
            unsafe {
                libc::kill(-pid.cast_signed(), libc::SIGKILL);
            }
        }
        let _ = child.wait().await;
        return Err(FyrerError::Task(TaskError::Cancelled(task_id.to_string())));
    }

    if let Some(pid) = child.id() {
        global::register_pid(task_id.clone(), pid);
    }

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| FyrerError::Task(TaskError::MissingStdout(task_id.to_string())))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| FyrerError::Task(TaskError::MissingStderr(task_id.to_string())))?;

    let log_sender = global::get().log_sender.clone();
    let stdout_reader = tokio::spawn(pipe_to_logger(
        stdout,
        task_id.clone(),
        log_sender.clone(),
        LogType::Info,
    ));
    let stderr_reader = tokio::spawn(pipe_to_logger(
        stderr,
        task_id.clone(),
        log_sender,
        LogType::Error,
    ));

    Ok(TaskProcess {
        task_id,
        child,
        stdout_reader: Some(stdout_reader),
        stderr_reader: Some(stderr_reader),
    })
}

async fn join<T>(handle: JoinHandle<FyrerResult<T>>, task: &TaskId) -> FyrerResult<T> {
    handle.await.map_err(|source| {
        FyrerError::Task(TaskError::Join {
            task: task.to_string(),
            source,
        })
    })?
}

async fn join_reader(
    handle: Option<JoinHandle<FyrerResult<()>>>,
    task: &TaskId,
) -> FyrerResult<()> {
    match handle {
        Some(handle) => join(handle, task).await,
        None => Ok(()),
    }
}

async fn pipe_to_logger(
    stream: impl AsyncRead + Unpin,
    task_id: TaskId,
    sender: mpsc::Sender<LogMessage>,
    log_type: LogType,
) -> FyrerResult<()> {
    let mut lines = tokio::io::BufReader::new(stream).lines();
    while let Some(line) = lines.next_line().await.map_err(|source| {
        FyrerError::Task(TaskError::ReadOutput {
            task: task_id.to_string(),
            source,
        })
    })? {
        sender
            .send(LogMessage {
                task_id: task_id.clone(),
                message: line,
                log_type,
            })
            .await
            .map_err(|source| {
                FyrerError::Logger(LoggerError::Send {
                    task: task_id.to_string(),
                    source,
                })
            })?;
    }
    Ok(())
}
