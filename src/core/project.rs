use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

use anyhow::Context;
use colored::Colorize;
use tokio::process::Command;

use crate::{config::Project, utils::env::parse_env_for_project};

impl Project {
    pub fn get_env_path(&self) -> PathBuf {
        Path::new(&self.root).join(self.env_path.clone().unwrap_or(".env".to_string()))
    }
    pub async fn run_task(&self, task: &str) -> anyhow::Result<()> {
        let task = self
            .tasks
            .get(task)
            .ok_or_else(|| anyhow::anyhow!("Task {} not found in project: {}", task, self.name))?;
        Ok(())
    }

    pub async fn setup(&self) -> anyhow::Result<()> {
        let cmd_str = &self
            .setup
            .clone()
            .unwrap_or_else(|| format!("echo \"No setup command for {}\"", &self.name));

        let mut env = parse_env_for_project(self);
        if let Some(project_env) = &self.env {
            env.extend(project_env.clone());
        }

        #[cfg(unix)]
        let mut cmd = {
            let mut c = Command::new("sh");
            c.arg("-c").arg(&cmd_str);
            c
        };

        #[cfg(windows)]
        let mut cmd = {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(&cmd_str);
            c
        };

        cmd.current_dir(&self.root);
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());
        if !env.is_empty() {
            cmd.envs(env);
        }

        let status = cmd
            .status()
            .await
            .with_context(|| format!("Failed to execute installer for: {}", &self.name))?;

        if status.success() {
            println!(
                "{} {}\n",
                "Installer completed successfully for:"
                    .bright_green()
                    .bold(),
                &self.name.bright_yellow()
            );
            return Ok(());
        }

        let exit_code = status.code().unwrap_or(-1);
        eprintln!(
            "{} {} {}",
            "Installer failed for:".bright_red().bold(),
            &self.name.bright_yellow(),
            format!("(Exit code: {exit_code})").bright_red()
        );
        println!();

        anyhow::bail!("setup command failed for project {}", &self.name);
    }
    pub async fn run_with_watch(&self) {}
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, fs};

    use tempfile::tempdir;

    use super::*;

    #[tokio::test]
    async fn test_installer_runs_setup_command() {
        let root = tempdir().unwrap();
        let marker_dir = root.path().join("setup-ran");
        let project = Project {
            name: "crux".to_string(),
            env_path: None,
            env: None,
            setup: Some("mkdir setup-ran".to_string()),
            root: root.path().display().to_string(),
            tasks: HashMap::new(),
        };
        project.setup().await.unwrap();

        assert!(marker_dir.is_dir());
    }

    #[tokio::test]
    async fn test_installer_uses_project_env() {
        let root = tempdir().unwrap();
        let env_file = root.path().join(".env");
        fs::write(&env_file, "SETUP_VAR=from-dotenv\n").unwrap();

        #[cfg(unix)]
        let setup = "test \"$SETUP_VAR\" = \"from-project\"";

        #[cfg(windows)]
        let setup = "if \"%SETUP_VAR%\"==\"from-project\" (exit /b 0) else (exit /b 1)";

        let project = Project {
            name: "crux".to_string(),
            env_path: None,
            env: Some(HashMap::from([(
                "SETUP_VAR".to_string(),
                "from-project".to_string(),
            )])),
            setup: Some(setup.to_string()),
            root: root.path().display().to_string(),
            tasks: HashMap::new(),
        };

        project.setup().await.unwrap();
    }
}
