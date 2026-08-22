use tokio::io::AsyncReadExt;

use crate::{logs::process::ProcessLog, process::child::ChildExitStatus, task::TaskId};

pub mod child;
pub mod command;
pub mod manager;

pub struct TaskProcess {
    task_id: TaskId,
    pid: Option<u32>,
    child: Option<tokio::process::Child>,
    log_tx: tokio::sync::mpsc::Sender<ProcessLog>,
    // the parent will listen to this for completion/failure and mark is accordingly
    exit_rx: tokio::sync::watch::Receiver<ChildExitStatus>,
    // the parent will send a signal to this channel to kill the process
    // TODO: decide the command enum
    command_tx: tokio::sync::mpsc::Sender<()>,
    #[cfg(unix)]
    pgrp: u32,
}

impl TaskProcess {
    pub fn spawn(
        command: command::ProcessCommand,
        task_id: TaskId,
        log_tx: tokio::sync::mpsc::Sender<ProcessLog>,
    ) -> Result<Self, std::io::Error> {
        let mut cmd: tokio::process::Command = command.into();
        let (exit_tx, exit_rx) = tokio::sync::watch::channel(ChildExitStatus::Running);
        let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(1);
        #[cfg(unix)]
        {
            cmd.process_group(0);
            let child = cmd.spawn()?;
            let pid = child.id();
            if let Some(pid) = pid {
                Ok(Self {
                    task_id,
                    pid: Some(pid),
                    child: Some(child),
                    log_tx,
                    pgrp: unsafe { libc::getpgid(pid as i32) as u32 },
                    command_tx,
                    exit_rx,
                })
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to spawn process",
                ))
            }
        }
    }

    pub fn id(&self) -> Option<u32> {
        self.pid
    }

    pub fn start_logging(&mut self) {
        let task_id_stdout = self.task_id.clone();
        let task_id_stderr = self.task_id.clone();
        let stdout = self.child.as_mut().and_then(|c| c.stdout.take());
        let stderr = self.child.as_mut().and_then(|c| c.stderr.take());
        let log_tx_stdout = self.log_tx.clone();
        let log_tx_stderr = self.log_tx.clone();

        tokio::spawn(async move {
            if let Some(mut stdout) = stdout {
                let mut buf = [0; 1024];
                loop {
                    match stdout.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            let _ = log_tx_stdout
                                .send(ProcessLog::Stdout {
                                    task_id: task_id_stdout.clone(),
                                    data: buf[..n].to_vec(),
                                })
                                .await;
                        }
                        Err(e) => {
                            eprintln!("Error reading stdout: {}", e);
                            break;
                        }
                    }
                }
            }
        });

        tokio::spawn(async move {
            if let Some(mut stderr) = stderr {
                let mut buf = [0; 1024];
                loop {
                    match stderr.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            let _ = log_tx_stderr
                                .send(ProcessLog::Stderr {
                                    task_id: task_id_stderr.clone(),
                                    data: buf[..n].to_vec(),
                                })
                                .await;
                        }
                        Err(e) => {
                            eprintln!("Error reading stderr: {}", e);
                            break;
                        }
                    }
                }
            }
        });
    }

    /// Takes ownership of the child and spawns a watcher that reaps it on exit,
    /// reporting the result as a [`ProcessLog::Exit`] event. This is the only
    /// place `child.wait()` is awaited.
    pub fn watch_exit(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        let task_id = self.task_id.clone();
        let log_tx = self.log_tx.clone();
        tokio::spawn(async move {
            let exit_code = match child.wait().await {
                Ok(status) => status.code().unwrap_or(-1),
                Err(_) => -1,
            };
            let _ = log_tx.send(ProcessLog::Exit { task_id, exit_code }).await;
        });
    }

    /// Terminates the process group. Reaping is left to the exit watcher, which
    /// reports the resulting [`ProcessLog::Exit`] event.
    pub fn kill(&mut self) -> Result<(), std::io::Error> {
        #[cfg(unix)]
        {
            if self.pid.is_none() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Process not running",
                ));
            }
            unsafe {
                libc::kill(-(self.pgrp as i32), libc::SIGKILL);
            }
        }
        Ok(())
    }
}
