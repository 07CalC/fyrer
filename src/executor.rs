use std::process::Stdio;

use crate::{
    error::{FyrerError, FyrerResult, logger::LoggerError, task::TaskError},
    global,
    logger::{LogMessage, LogType},
    tasks::{Task, TaskId},
};
use tokio::{io::AsyncBufReadExt, process::Command};

pub async fn execute_tasks(tasks: &[TaskId]) -> FyrerResult<()> {
    let state = global::get();
    let order = state.task_graph.get_orders(tasks)?;
    for batch in order {
        let mut handles = vec![];
        for task_id in batch {
            let task = state
                .task_map
                .get(&task_id)
                .cloned()
                .ok_or_else(|| FyrerError::Task(TaskError::NotFound(task_id.to_string())))?;
            handles.push((
                task_id.to_string(),
                tokio::spawn(async move { execute_task(task).await }),
            ));
        }
        for (task_name, handle) in handles {
            join_reader(handle, &task_name).await?;
        }
    }
    Ok(())
}

pub async fn execute_task(task: Task) -> FyrerResult<()> {
    let task_id = TaskId::new(&task.project_name, &task.task_name);
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(&task.cmd);
    cmd.current_dir(&task.project_root);
    cmd.envs(&task.env);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        FyrerError::Task(TaskError::Spawn {
            task: task_id.to_string(),
            source: e,
        })
    })?;
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

    let status = child.wait().await.map_err(|e| {
        FyrerError::Task(TaskError::Wait {
            task: task_id.to_string(),
            source: e,
        })
    })?;
    join_reader(stdout_reader, &task_id.to_string()).await?;
    join_reader(stderr_reader, &task_id.to_string()).await?;

    if !status.success() {
        return Err(FyrerError::Task(TaskError::Failed {
            task: task_id.to_string(),
            code: status.code().unwrap_or(-1),
        }));
    }

    Ok(())
}

async fn join_reader(
    handle: tokio::task::JoinHandle<FyrerResult<()>>,
    task: &str,
) -> FyrerResult<()> {
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
