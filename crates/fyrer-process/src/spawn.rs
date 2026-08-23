//! OS process primitives: shell command building and process-group kills.
//!
//! Unix uses process groups (the child becomes a group leader, so the whole
//! tree dies together). Windows gets `CREATE_NEW_PROCESS_GROUP` as best-effort;
//! Job Objects are a tracked future improvement, isolated to this crate.

use std::process::Stdio;

use fyrer_core::spec::TaskSpec;

/// Build a tokio Command running `spec.cmd` through the platform shell
/// (`sh -c` on unix, `cmd /C` on Windows), with piped stdio and its own
/// process group.
pub fn build_command(spec: &TaskSpec) -> tokio::process::Command {
    #[cfg(unix)]
    let mut cmd = {
        use tokio::process::Command;
        let mut c = Command::new("sh");
        c.arg("-c").arg(&spec.cmd);
        c
    };
    #[cfg(windows)]
    let mut cmd = {
        use tokio::process::Command;
        let mut c = Command::new("cmd");
        c.arg("/C").arg(&spec.cmd);
        c
    };

    cmd.current_dir(&spec.cwd);
    cmd.envs(&spec.env);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.stdin(Stdio::piped());

    // New process group (unix) / new process group flag (windows) so the
    // whole child tree can be killed at once. See module docs for Windows
    // limitations.
    #[cfg(unix)]
    cmd.process_group(0);
    #[cfg(windows)]
    cmd.creation_flags(0x00000200); // CREATE_NEW_PROCESS_GROUP

    cmd
}

/// Kill the child's entire process group, then the child itself.
pub async fn kill_process_group(child: &mut tokio::process::Child) {
    #[cfg(unix)]
    if let Some(pid) = child.id() {
        // Negative pid targets the process group led by the child.
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
    let _ = child.kill().await;
}
