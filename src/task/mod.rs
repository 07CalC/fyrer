use std::{path::PathBuf, process::Stdio, time::Duration};

use tokio::process::Command;

use crate::{TaskId, env::EnvMap};

mod graph;
mod id;
mod map;
mod spawn;

#[derive(Debug, Clone)]
pub struct Task {
    pub id: TaskId,
    pub env: EnvMap,
    pub cache: bool,
    pub watch: bool,
    pub persistent: bool,
    pub timeout: Option<Duration>,
    pub cwd: PathBuf,
    pub cmd: String,
    pub depends_on: Vec<TaskId>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub ignore: Vec<String>,
}

impl Task {
    pub fn new(
        id: TaskId,
        env: EnvMap,
        cache: bool,
        watch: bool,
        persistent: bool,
        timeout: Option<Duration>,
        cwd: PathBuf,
        cmd: String,
        depends_on: Vec<TaskId>,
        inputs: Vec<String>,
        outputs: Vec<String>,
        ignore: Vec<String>,
    ) -> Self {
        Self {
            id,
            env,
            cache,
            watch,
            persistent,
            timeout,
            cwd,
            cmd,
            depends_on,
            inputs,
            outputs,
            ignore,
        }
    }

    fn command(&self) -> Command {
        #[cfg(unix)]
        let mut command = Command::new("sh");
        #[cfg(windows)]
        let mut command = Command::new("cmd");

        #[cfg(unix)]
        command.arg("-c").arg(&self.cmd);
        #[cfg(windows)]
        command.arg("/C").arg(&self.cmd);

        command.current_dir(&self.cwd);
        command.envs(&self.env);
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        command.stdin(Stdio::piped());
        // setting the pgid to 0 will make the child process the leader of a new process
        // this is important because we want to be able to kill the entire process group
        // when the task is stopped
        command.process_group(0);

        command
    }
}
