use std::process::Stdio;
use std::time::Duration;

use crate::{
    config::RestartStrategy,
    error::{FyrerError, FyrerResult, logger::LoggerError, task::TaskError},
    global,
    logger::{LogMessage, LogType},
    tasks::{Task, TaskId},
};
use tokio::{io::AsyncBufReadExt, process::Command};

pub struct TaskProcess {
    pub task_id: TaskId,
    child: tokio::process::Child,
    stdout_reader: Option<tokio::task::JoinHandle<FyrerResult<()>>>,
    stderr_reader: Option<tokio::task::JoinHandle<FyrerResult<()>>>,
}

impl TaskProcess {
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

    #[cfg(unix)]
    fn signal_group(&self, signal: libc::c_int) {
        if let Some(pid) = self.child.id() {
            unsafe {
                libc::kill(-(pid as libc::pid_t), signal);
            }
        }
    }
}

impl Drop for TaskProcess {
    fn drop(&mut self) {
        self.signal_group(libc::SIGKILL);
        global::unregister_pid(&self.task_id);
    }
}

pub async fn execute_tasks(tasks: &[TaskId]) -> FyrerResult<Vec<(Task, TaskProcess)>> {
    let state = global::get();
    let order = state.task_graph.get_orders(tasks)?;
    let mut running = vec![];
    for batch in order {
        let mut handles = vec![];
        for task_id in batch {
            let task = state
                .task_map
                .get(&task_id)
                .cloned()
                .ok_or_else(|| FyrerError::Task(TaskError::NotFound(task_id.to_string())))?;
            if task.restart.strategy == RestartStrategy::FileChange {
                handles.push((
                    task_id.to_string(),
                    tokio::spawn(async move { start_task(task).await.map(Some) }),
                ));
            } else {
                handles.push((
                    task_id.to_string(),
                    tokio::spawn(async move { execute_task(task).await.map(|_| None) }),
                ));
            }
        }
        for (task_name, handle) in handles {
            if let Some(process) = join(handle, &task_name).await? {
                let id = process.task_id.clone();
                let task = state
                    .task_map
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| FyrerError::Task(TaskError::NotFound(id.to_string())))?;
                running.push((task, process));
            }
        }
    }
    Ok(running)
}

pub async fn execute_task(task: Task) -> FyrerResult<()> {
    let mut process = start_task(task).await?;
    let task_id = process.task_id.clone();
    let status = process.child.wait().await.map_err(|e| {
        FyrerError::Task(TaskError::Wait {
            task: task_id.to_string(),
            source: e,
        })
    })?;
    join(process.stdout_reader.take().unwrap(), &task_id.to_string()).await?;
    join(process.stderr_reader.take().unwrap(), &task_id.to_string()).await?;

    if !status.success() {
        return Err(FyrerError::Task(TaskError::Failed {
            task: task_id.to_string(),
            code: status.code().unwrap_or(-1),
        }));
    }

    Ok(())
}

pub async fn start_task(task: Task) -> FyrerResult<TaskProcess> {
    let task_id = TaskId::new(&task.project_name, &task.task_name);
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(&task.cmd);
    cmd.current_dir(&task.project_root);
    cmd.envs(&task.env);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    #[cfg(unix)]
    cmd.process_group(0);

    let mut child = cmd.spawn().map_err(|e| {
        FyrerError::Task(TaskError::Spawn {
            task: task_id.to_string(),
            source: e,
        })
    })?;
    if global::is_shutting_down() {
        if let Some(pid) = child.id() {
            #[cfg(unix)]
            unsafe {
                libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
            }
        }
        let _ = child.wait().await;
        return Err(FyrerError::Task(TaskError::Cancelled(task_id.to_string())));
    }
    if let Some(pid) = child.id() {
        global::register_pid(task_id.clone(), pid);
    }
    let stdout = child.stdout.take().ok_or_else(|| {
        FyrerError::Task(TaskError::MissingStdout(task_id.to_string()))
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        FyrerError::Task(TaskError::MissingStderr(task_id.to_string()))
    })?;

    let stdout_reader: tokio::task::JoinHandle<FyrerResult<()>> = tokio::spawn(pipe_to_logger(
        stdout,
        task_id.clone(),
        global::get().log_sender.clone(),
        LogType::Info,
    ));
    let stderr_reader: tokio::task::JoinHandle<FyrerResult<()>> = tokio::spawn(pipe_to_logger(
        stderr,
        task_id.clone(),
        global::get().log_sender.clone(),
        LogType::Error,
    ));

    Ok(TaskProcess {
        task_id,
        child,
        stdout_reader: Some(stdout_reader),
        stderr_reader: Some(stderr_reader),
    })
}

async fn join<T>(
    handle: tokio::task::JoinHandle<FyrerResult<T>>,
    task: &str,
) -> FyrerResult<T> {
    handle
        .await
        .map_err(|e| FyrerError::Task(TaskError::Join {
            task: task.to_string(),
            source: e,
        }))?
}

async fn pipe_to_logger(
    stream: impl tokio::io::AsyncRead + Unpin,
    task_id: TaskId,
    sender: tokio::sync::mpsc::Sender<LogMessage>,
    log_type: LogType,
) -> FyrerResult<()> {
    let mut lines = tokio::io::BufReader::new(stream).lines();
    while let Some(line) = lines.next_line().await.map_err(|e| {
        FyrerError::Task(TaskError::ReadOutput {
            task: task_id.to_string(),
            source: e,
        })
    })? {
        sender
            .send(LogMessage {
                task_id: task_id.clone(),
                message: line,
                log_type,
            })
            .await
            .map_err(|e| FyrerError::Logger(LoggerError::Send {
                task: task_id.to_string(),
                source: e,
            }))?;
    }
    Ok(())
}
