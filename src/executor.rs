use std::process::Stdio;

use crate::{
    error::FyrerResult,
    global,
    logger::LogMessage,
    tasks::{Task, TaskId},
};
use tokio::{io::AsyncBufReadExt, process::Command};

pub async fn execute_tasks(task_name: &str) -> FyrerResult<()> {
    let state = global::get();
    let order = state.task_graph.get_order(task_name)?;
    for batch in order {
        let mut handles = vec![];
        for task_id in batch {
            let task = state
                .task_map
                .get(&task_id)
                .expect("Task not found in task map. This should not happen if the graph is valid.")
                .clone();
            let handle = tokio::spawn(async move { execute_task(task).await });
            handles.push(handle);
        }
        for handle in handles {
            handle.await.expect("Task panicked")?;
        }
    }
    Ok(())
}

pub async fn execute_task(task: Task) -> FyrerResult<()> {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(&task.cmd);
    cmd.current_dir("/home/calc/Documents/fyrer/");
    cmd.envs(&task.env);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| {
            println!(
                "Failed to spawn command for task {:?}: {}",
                TaskId::new(&task.project_name, &task.task_name),
                e.to_string()
            );
        })
        .expect("Failed to spawn command");
    let stdout = child.stdout.take().expect("Failed to capture stdout");
    let stderr = child.stderr.take().expect("Failed to capture stderr");

    let task_id_out = TaskId::new(&task.project_name, &task.task_name);
    let task_id_err = task_id_out.clone();
    let logger_out = global::get().log_sender.clone();
    let logger_err = global::get().log_sender.clone();
    let stdout_reader = tokio::spawn(async move {
        let mut stdout_reader = tokio::io::BufReader::new(stdout).lines();
        while let Some(line) = stdout_reader.next_line().await.unwrap_or(None) {
            logger_out
                .send(LogMessage {
                    task_id: task_id_out.clone(),
                    message: line.clone(),
                    log_type: crate::logger::LogType::Info,
                })
                .await
                .expect("Failed to send log message");
        }
    });
    let stderr_reader = tokio::spawn(async move {
        let mut stderr_reader = tokio::io::BufReader::new(stderr).lines();
        while let Some(line) = stderr_reader.next_line().await.unwrap_or(None) {
            logger_err
                .send(LogMessage {
                    task_id: task_id_err.clone(),
                    message: line.clone(),
                    log_type: crate::logger::LogType::Error,
                })
                .await
                .expect("Failed to send log message");
        }
    });

    let status = child.wait().await.expect("Failed to wait for task");
    let _ = stdout_reader.await;
    let _ = stderr_reader.await;
    if !status.success() {
        eprintln!(
            "Task {:?} failed with status: {}",
            TaskId::new(&task.project_name, &task.task_name),
            status
        );
    }

    Ok(())
}
