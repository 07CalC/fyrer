pub mod command;
pub mod manager;

pub struct TaskProcess {
    task_id: crate::task::TaskId,
    pid: Option<u32>,
    child: tokio::process::Child,
    log_tx: tokio::sync::mpsc::Sender<crate::logs::process::ProcessLog>,
    #[cfg(unix)]
    pgrp: u32,
}

impl TaskProcess {
    pub fn spawn(
        command: command::ProcessCommand,
        task_id: TaskId,
        log_tx: tokio::sync::mpsc::Sender<crate::logs::process::ProcessLog>,
    ) -> Result<Self, std::io::Error> {
        let mut cmd: tokio::process::Command = command.into();
        #[cfg(unix)]
        {
            cmd.process_group(0);
            let child = cmd.spawn()?;
            let pid = child.id();
            if let Some(pid) = pid {
                Ok(Self {
                    task_id,
                    pid: Some(pid),
                    child,
                    log_tx,
                    pgrp: unsafe { libc::getpgid(pid as i32) as u32 },
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

    pub fn kill(&mut self) -> Result<(), std::io::Error> {
        #[cfg(unix)]
        {
            if let Some(pid) = self.pid {
                let pgrp = self.pgrp;
                unsafe {
                    libc::kill(-pgrp as i32, libc::SIGKILL);
                }
                self.child.kill()?;
                self.child.wait()?;
            } else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Process not running",
                ));
            }
        }
        Ok(())
    }

    pub fn wait(&mut self) -> Result<std::process::ExitStatus, std::io::Error> {
        self.child.wait()?
    }

    pub fn start_logging(&mut self) {
        let task_id = self.task_id;
        let mut stdout = self.child.stdout.take();
        let mut stderr = self.child.stderr.take();
        let log_tx = self.log_tx.clone();

        tokio::spawn(async move {
            if let Some(mut stdout) = stdout {
                let mut buf = [0; 1024];
                loop {
                    match stdout.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            let _ = log_tx
                                .send(crate::logs::process::ProcessLog::Stdout {
                                    task_id,
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
                            let _ = log_tx
                                .send(crate::logs::process::ProcessLog::Stderr {
                                    task_id,
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
}
