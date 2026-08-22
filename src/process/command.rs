use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct ProcessCommand {
    cmd: OsString,
    args: Vec<OsString>,
    cwd: Option<PathBuf>,
    env: BTreeMap<OsString, OsString>,
}

impl ProcessCommand {
    pub fn new(cmd: impl AsRef<OsStr>) -> Self {
        Self {
            cmd: cmd.as_ref().to_os_string(),
            args: Vec::new(),
            cwd: None,
            env: BTreeMap::new(),
        }
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args = args
            .into_iter()
            .map(|s| s.as_ref().to_os_string())
            .collect();
        self
    }

    pub fn cwd<P: AsRef<Path>>(mut self, cwd: P) -> Self {
        self.cwd = Some(cwd.as_ref().to_path_buf());
        self
    }

    pub fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.env
            .insert(key.as_ref().to_os_string(), value.as_ref().to_os_string());
        self
    }

    pub fn envs<I, K, V>(mut self, envs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        for (key, value) in envs {
            self.env
                .insert(key.as_ref().to_os_string(), value.as_ref().to_os_string());
        }
        self
    }
}

impl From<ProcessCommand> for tokio::process::Command {
    fn from(value: ProcessCommand) -> Self {
        let mut cmd = tokio::process::Command::new(value.cmd);
        cmd.args(value.args);
        if let Some(cwd) = value.cwd {
            cmd.current_dir(cwd);
        }
        cmd.envs(value.env);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd.stdin(std::process::Stdio::piped());
        cmd
    }
}
