use std::{
    fmt,
    hash::{DefaultHasher, Hash, Hasher},
    path::PathBuf,
    process::Stdio,
};

use anyhow::{Result, anyhow};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::mpsc::Sender,
    task::JoinHandle,
};

use crate::{config::RestartConfig, events::TaskCommand};
use crate::{env::EnvMap, events::AppEvent};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskId {
    project_name: String,
    task_name: String,
}

#[derive(Debug, Clone)]
pub enum TaskStatus {
    Running,
    Waiting,
    Complete,
    Failed { code: i32, error: Option<String> },
    Restarting,
}

#[derive(Debug)]
pub struct Task {
    pub project_name: String,
    pub project_root: PathBuf,
    pub env: EnvMap,
    pub task_name: String,
    pub cmd: String,
    pub depends_on: Vec<String>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub ignore: Vec<String>,
    pub cache: bool,
    pub restart: RestartConfig,
}

#[derive(Debug)]
pub struct SpawnedTask {
    pub handle: JoinHandle<()>,
    pub command_sender: Sender<TaskCommand>,
}

impl Task {
    #[must_use]
    pub fn id(&self) -> TaskId {
        TaskId::new(&self.project_name, &self.task_name)
    }

    pub fn spawn(&self, event_bus_sender: Sender<AppEvent>) -> Result<SpawnedTask> {
        let (command_sender, mut command_receiver) = tokio::sync::mpsc::channel(1);
        let task_id = self.id();
        let mut child = self
            .build_command()
            .process_group(0)
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn task {}: {}", task_id, e))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Failed to capture stdout for task {}", task_id))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("Failed to capture stderr for task {}", task_id))?;
        let mut stdin = child.stdin.take();

        // pipe stdout
        let tx = event_bus_sender.clone();
        let id = task_id.clone();

        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx
                    .send(AppEvent::Stdout {
                        task_id: id.clone(),
                        line,
                    })
                    .await;
            }
        });

        // pipe stderr
        let tx = event_bus_sender.clone();
        let id = task_id.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx
                    .send(AppEvent::Stderr {
                        task_id: id.clone(),
                        line,
                    })
                    .await;
            }
        });

        // main loop, owns the child and handle stdin
        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    status = child.wait() => {
                        let event = match status {
                            Ok(s) if s.success() => AppEvent::TaskComplete { task_id: task_id.clone() },
                            Ok(s) => AppEvent::TaskFailed {
                                task_id: task_id.clone(),
                                exit_code: s.code().unwrap_or(-1),
                                error: None,
                            },
                            Err(e) => AppEvent::TaskFailed {
                                task_id: task_id.clone(),
                                exit_code: -1,
                                error: Some(format!("Failed to wait for task {}: {}", task_id, e)),
                            },
                        };
                        let _ = event_bus_sender.send(event).await;
                        break;
                    }
                    Some(cmd) = command_receiver.recv() => {
                        match cmd {
                            TaskCommand::Stdin(input) => {
                                if let Some(ref mut w) = stdin {
                                    let _= w.write_all(input.as_bytes()).await;
                                    let _ = w.flush().await;
                                }
                            }
                            TaskCommand::Kill => {
                                #[cfg(unix)]
                                if let Some(pid) = child.id() {
                                    unsafe {
                                        libc::kill(-(pid as i32), libc::SIGKILL);
                                    }
                                }
                                let _ = child.kill().await;
                                // we don't break here, because we want to wait for the
                                // child to exit and send the event in the next
                                // iteration of the loop via child.wait() branch
                            }
                        }
                    }
                }
            }
        });
        Ok(SpawnedTask {
            handle,
            command_sender,
        })
    }

    fn build_command(&self) -> Command {
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg(&self.cmd)
            .current_dir(&self.project_root)
            .envs(&self.env)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped());
        command
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.project_name, self.task_name)
    }
}

impl TaskId {
    #[must_use]
    pub fn new(project_name: &str, task_name: &str) -> Self {
        Self {
            project_name: project_name.to_string(),
            task_name: task_name.to_string(),
        }
    }

    #[must_use]
    pub fn project_name(&self) -> &str {
        &self.project_name
    }
    #[must_use]
    pub fn task_name(&self) -> &str {
        &self.task_name
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let (project, task) = s.split_once(':')?;
        if project.is_empty() || task.is_empty() || task.contains(':') {
            return None;
        }
        Some(Self::new(project, task))
    }

    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn hash(&self) -> usize {
        let mut hasher = DefaultHasher::new();
        Hash::hash(self, &mut hasher);
        hasher.finish() as usize
    }
}
