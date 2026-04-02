use std::process::Stdio;
use tokio::process::Child;
use tokio::process::Command;

pub async fn kill_process(child: &mut Child) {
    if let Some(pid) = child.id() {
        #[cfg(unix)]
        {
            let output = Command::new("ps")
                .arg("-o")
                .arg("pid=")
                .arg("--ppid")
                .arg(pid.to_string())
                .output()
                .await;

            match output {
                Ok(output) => {
                    let pids = String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .filter_map(|l| l.trim().parse::<u32>().ok())
                        .collect::<Vec<_>>();

                    for child_pid in pids {
                        let _ = Command::new("kill")
                            .arg("-9")
                            .arg(child_pid.to_string())
                            .status()
                            .await;
                    }

                    let _ = Command::new("kill")
                        .arg("-9")
                        .arg(pid.to_string())
                        .status()
                        .await;
                }
                Err(_) => {
                    let _ = Command::new("kill")
                        .arg("-9")
                        .arg(pid.to_string())
                        .status()
                        .await;
                }
            }
        }

        #[cfg(windows)]
        {
            let _ = Command::new("taskkill")
                .arg("/PID")
                .arg(pid.to_string())
                .arg("/T")
                .arg("/F")
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()
                .await;
        }
    }
}
