use crate::cli::logger::{Log, LogKind};
use crate::config::Service;
use crate::env_parser::parse_env;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::Stdio;
use tokio::io::AsyncBufReadExt;
use tokio::process::{Child, Command};
use tokio::sync::mpsc::Sender;

pub async fn spawn_service(
    service: &Service,
    color: colored::Color,
    wait: bool,
    logger: Sender<Log>,
) -> Option<Child> {
    #[cfg(unix)]
    let mut cmd = Command::new("sh");
    #[cfg(unix)]
    cmd.arg("-c").arg(&service.cmd);

    #[cfg(windows)]
    let mut cmd = Command::new("cmd");
    #[cfg(windows)]
    cmd.arg("/C").arg(&service.cmd);

    cmd.current_dir(&service.dir);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let dotenv_path = if let Some(env_path) = &service.env_path {
        Path::new(&service.dir).join(env_path)
    } else {
        Path::new(&service.dir).join(".env")
    };

    if let Ok(mut dotenv) = File::open(&dotenv_path) {
        let mut content = String::new();
        if dotenv.read_to_string(&mut content).is_ok() {
            let envs = parse_env(&content);
            for (k, v) in envs {
                cmd.env(k, v);
            }
        }
    }

    // -------- Inline env --------
    if let Some(envs) = &service.env {
        for (key, value) in envs {
            cmd.env(key, value);
        }
    }

    let _ = logger
        .send(Log {
            service: service.name.clone(),
            message: format!("Starting service... {}", service.cmd),
            kind: LogKind::System,
            color,
        })
        .await;

    let mut child = cmd.spawn().ok()?;

    let stdout = child.stdout.take().expect("Failed to capture stdout");
    let stderr = child.stderr.take().expect("Failed to capture stderr");

    let quiet = service.quiet.unwrap_or(false);

    if !quiet {
        let service_name = service.name.clone();

        let mut stdout_reader = tokio::io::BufReader::new(stdout).lines();
        let mut stderr_reader = tokio::io::BufReader::new(stderr).lines();

        {
            let logger = logger.clone();
            let name = service_name.clone();

            tokio::spawn(async move {
                while let Ok(Some(line)) = stdout_reader.next_line().await {
                    let _ = logger
                        .send(Log {
                            service: name.clone(),
                            message: line,
                            kind: LogKind::Output,
                            color,
                        })
                        .await;
                }
            });
        }

        {
            let logger = logger.clone();
            let name = service_name.clone();

            tokio::spawn(async move {
                while let Ok(Some(line)) = stderr_reader.next_line().await {
                    let _ = logger
                        .send(Log {
                            service: name.clone(),
                            message: line,
                            kind: LogKind::Error,
                            color,
                        })
                        .await;
                }
            });
        }
    }

    if wait {
        let _ = child.wait().await;

        let _ = logger
            .send(Log {
                service: service.name.clone(),
                message: "Service exited".into(),
                kind: LogKind::System,
                color,
            })
            .await;

        return None;
    }

    Some(child)
}
