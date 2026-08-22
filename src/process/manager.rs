use std::collections::HashMap;

use crate::{
    logs::process::ProcessLog,
    process::{TaskProcess, command::ProcessCommand},
    task::{Task, TaskId},
};

pub struct ProcessManager {
    children: HashMap<TaskId, TaskProcess>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            children: HashMap::new(),
        }
    }

    /// Spawns a task's process and takes full ownership of it: pipes its output
    /// and reports its exit through `log_tx`.
    pub fn spawn(
        &mut self,
        task: &Task,
        log_tx: tokio::sync::mpsc::Sender<ProcessLog>,
    ) -> Result<(), std::io::Error> {
        #[cfg(unix)]
        let command = ProcessCommand::new("sh")
            .args(&["-c", &task.cmd])
            .cwd(task.cwd.clone())
            .envs(task.env.clone());
        #[cfg(windows)]
        let command = ProcessCommand::new("cmd")
            .args(&["/C", &task.cmd])
            .cwd(task.cwd())
            .envs(task.env());

        let mut task_process = TaskProcess::spawn(command, task.id.clone(), log_tx)?;
        task_process.start_logging();
        task_process.watch_exit();
        self.children.insert(task.id.clone(), task_process);
        Ok(())
    }

    /// Terminates the task's process group. Its exit is reported asynchronously
    /// through the log channel; the task is removed from management immediately.
    pub fn kill(&mut self, task_id: &TaskId) -> Result<(), std::io::Error> {
        if let Some(mut child) = self.children.remove(task_id) {
            child.kill()?;
        }
        Ok(())
    }

    pub fn kill_all(&mut self) {
        for (_, mut child) in self.children.drain() {
            let _ = child.kill();
        }
    }

    pub fn restart(
        &mut self,
        task: &Task,
        log_tx: tokio::sync::mpsc::Sender<ProcessLog>,
    ) -> Result<(), std::io::Error> {
        self.kill(&task.id.clone())?;
        self.spawn(task, log_tx)
    }
}