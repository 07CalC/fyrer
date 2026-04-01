use crate::config::Project;
use crate::utils::env::parse_env_for_project;
use colored::Colorize;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::Stdio;
use tokio::io::AsyncBufReadExt;
use tokio::process::{Child, Command};

pub async fn spawn_service(
    service: &Project,
    color: colored::Color,
    wait: bool,
    max_name_len: usize,
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
    let dotenv_path;
    if let Some(env_path) = &service.env_path {
        dotenv_path = Path::new(&service.dir).join(env_path);
    } else {
        dotenv_path = Path::new(&service.dir).join(".env");
    }

    if let Ok(mut dotenv) = File::open(&dotenv_path) {
        let mut content = String::new();
        if let Err(_) = dotenv.read_to_string(&mut content) {
        } else {
            let envs = parse_env_for_project(&service);
            for (k, v) in envs.into_iter() {
                cmd.env(k, v);
            }
        }
    }

    if let Some(envs) = &service.env {
        for (key, value) in envs {
            cmd.env(key, value);
        }
    }

    let out_prefix = format!("[{}]", service.name);
    let padded_name = format!("{:<width$}", out_prefix.clone(), width = max_name_len);

    println!(
        "├─{} ➤ Starting service... {}",
        padded_name.color(color).bold(),
        service.cmd.bright_white()
    );

    let mut child = cmd.spawn().ok()?;
    let stdout = child.stdout.take().expect("Failed to capture stdout");
    let stderr = child.stderr.take().expect("Failed to capture stderr");

    let quiet = service.quiet.unwrap_or(false);

    if !quiet {
        let name = service.name.clone();
        let mut stdout_reader = tokio::io::BufReader::new(stdout).lines();
        let mut stderr_reader = tokio::io::BufReader::new(stderr).lines();

        let name_prefix = format!("[{}] ", name).color(color).bold();
        let out_prefix = name_prefix.clone();
        let err_prefix = name_prefix.clone();

        tokio::spawn(async move {
            while let Ok(Some(line)) = stdout_reader.next_line().await {
                let padded_name = format!("{:<width$}", out_prefix.clone(), width = max_name_len);
                println!(
                    "├─{} ➤ {}",
                    padded_name.bright_cyan().bold(),
                    line.color(color)
                );
            }
        });

        tokio::spawn(async move {
            while let Ok(Some(line)) = stderr_reader.next_line().await {
                let padded_name = format!("{:<width$}", err_prefix.clone(), width = max_name_len);
                eprintln!(
                    "├─{} ⚠ {}",
                    padded_name.bright_red().bold(),
                    line.bright_red().bold()
                );
            }
        });
    }

    if wait {
        let _ = child.wait().await;
        return None;
    }

    Some(child)
}
