use std::process::Stdio;

use crate::{
    error::{FyrerError, FyrerResult, logger::LoggerError, task::TaskError},
    global,
    logger::LogMessage,
    tasks::{Task, TaskId},
};
use tokio::{io::AsyncBufReadExt, process::Command};

pub async fn execute_tasks(tasks: &[TaskId]) -> FyrerResult<()> {
    let state = global::get();
    let order = state.task_graph.get_orders(tasks)?;
    for batch in order {
        let mut handles = vec![];
        for task_id in batch {
            let task = state.task_map.get(&task_id).cloned().ok_or_else(|| {
                FyrerError::Task(TaskError::NotFound(task_id.to_string()))
            })?;
            let handle = tokio::spawn(async move { execute_task(task).await });
            handles.push((task_id.to_string(), handle));
        }
        for (task_name, handle) in handles {
            let result = handle
                .await
                .map_err(|e| FyrerError::Task(TaskError::Join {
                    task: task_name,
                    source: e,
                }))?;
            result?;
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

    let task_id_out = task_id.clone();
    let task_id_err = task_id.clone();
    let logger_out = global::get().log_sender.clone();
    let logger_err = global::get().log_sender.clone();
    let stdout_reader: tokio::task::JoinHandle<FyrerResult<()>> = tokio::spawn(async move {
        let mut stdout_reader = tokio::io::BufReader::new(stdout).lines();
        while let Some(line) = stdout_reader.next_line().await.map_err(|e| {
            FyrerError::Task(TaskError::ReadOutput {
                task: task_id_out.to_string(),
                source: e,
            })
        })? {
            logger_out
                .send(LogMessage {
                    task_id: task_id_out.clone(),
                    message: line,
                    log_type: crate::logger::LogType::Info,
                })
                .await
                .map_err(|e| {
                    FyrerError::Logger(LoggerError::Send {
                        task: task_id_out.to_string(),
                        source: e,
                    })
                })?;
        }
        Ok(())
    });
    let stderr_reader: tokio::task::JoinHandle<FyrerResult<()>> = tokio::spawn(async move {
        let mut stderr_reader = tokio::io::BufReader::new(stderr).lines();
        while let Some(line) = stderr_reader.next_line().await.map_err(|e| {
            FyrerError::Task(TaskError::ReadOutput {
                task: task_id_err.to_string(),
                source: e,
            })
        })? {
            logger_err
                .send(LogMessage {
                    task_id: task_id_err.clone(),
                    message: line,
                    log_type: crate::logger::LogType::Error,
                })
                .await
                .map_err(|e| {
                    FyrerError::Logger(LoggerError::Send {
                        task: task_id_err.to_string(),
                        source: e,
                    })
                })?;
        }
        Ok(())
    });

    let status = child.wait().await.map_err(|e| {
        FyrerError::Task(TaskError::Wait {
            task: task_id.to_string(),
            source: e,
        })
    })?;
    let stdout_result = stdout_reader.await.map_err(|e| {
        FyrerError::Task(TaskError::Join {
            task: task_id.to_string(),
            source: e,
        })
    })?;
    stdout_result?;
    let stderr_result = stderr_reader.await.map_err(|e| {
        FyrerError::Task(TaskError::Join {
            task: task_id.to_string(),
            source: e,
        })
    })?;
    stderr_result?;

    if !status.success() {
        return Err(FyrerError::Task(TaskError::Failed {
            task: task_id.to_string(),
            code: status.code().unwrap_or(-1),
        }));
    }

    Ok(())
}
