use std::collections::HashMap;

use crate::{
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

    pub fn kill_all(&mut self) {
        for (_, child) in self.children.iter_mut() {
            let _ = child.kill();
        }
        self.children.clear();
    }

    pub fn kill(&mut self, task_id: &TaskId) -> Result<(), std::io::Error> {
        if let Some(mut child) = self.children.remove(task_id) {
            child.kill()?;
        }
        Ok(())
    }

    pub fn spawn(
        &mut self,
        task: &Task,
        log_tx: tokio::sync::mpsc::Sender<crate::logs::process::ProcessLog>,
    ) -> Result<(), std::io::Error> {
        let task_id = task.id();
        #[cfg(unix)]
        let mut command = ProcessCommand::new("sh")
            .args(&["-c", &task.cmd])
            .cwd(task.cwd)
            .envs(task.env);
        #[cfg(windows)]
        let mut command = ProcessCommand::new("cmd")
            .args(&["/C", &task.cmd])
            .cwd(task.cwd())
            .envs(task.env());

        let task_process = TaskProcess::spawn(command, task_id, log_tx)?;
        self.children.insert(task_id, task_process);
        Ok(())
    }

    pub fn restart(
        &mut self,
        task: &Task,
        log_tx: tokio::sync::mpsc::Sender<crate::logs::process::ProcessLog>,
    ) -> Result<(), std::io::Error> {
        self.kill(&task.id())?;
        self.spawn(task, log_tx)
    }
}
