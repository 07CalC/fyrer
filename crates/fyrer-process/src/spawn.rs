use std::{path::PathBuf, process::Stdio};

use anyhow::Result;
use fyrer_core::spec::TaskSpec;
use tokio::process::Command;

pub fn build_command(spec: &TaskSpec) -> Command {
    #[cfg(unix)]
    let mut cmd = Command::new("sh");
    #[cfg(windows)]
    let mut cmd = Command::new("cmd");

    #[cfg(unix)]
    cmd.arg("-c").arg(&spec.cmd);
    #[cfg(windows)]
    cmd.arg("/C").arg(&spec.cmd);

    cmd.current_dir(&spec.cwd);
    cmd.envs(&spec.env);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.stdin(Stdio::piped());

    #[cfg(unix)]
    cmd.process_group(0);
    #[cfg(windows)]
    cmd.creation_flags(0x00000200);

    cmd
}

pub async fn kill_process_group(child: &mut tokio::process::Child) {
    #[cfg(unix)]
    if let Some(pid) = child.id() {
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
    let _ = child.kill().await;
}

#[allow(dead_code)]
pub fn build_command_from_parts(cwd: &PathBuf, env: &std::collections::HashMap<String, String>, cmd: &str) -> Command {
    let spec = fyrer_core::spec::TaskSpec::new(
        fyrer_core::TaskId::new("tmp", "tmp"),
        env.clone(),
        false,
        false,
        false,
        None,
        cwd.clone(),
        cmd.to_string(),
        vec![],
        vec![],
        vec![],
        vec![],
    );
    build_command(&spec)
}
