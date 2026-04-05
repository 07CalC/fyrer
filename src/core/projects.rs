use colored::*;
use std::{collections::HashMap, path::Path};

use crate::{config::types::Project, env::loader::load_env_from_file};

impl Project {
    pub fn setup(&self) {
        println!(
            "{}",
            "────────────────────────────────────────────".bright_black()
        );

        println!(
            "{} {}",
            "🔹 Running setup for project:".bright_blue().bold(),
            self.name.bright_yellow()
        );

        if self.setup.is_none() {
            println!("{}", "⚡ No setup command defined.".bright_yellow().bold());
            println!(
                "{}",
                "────────────────────────────────────────────".bright_black()
            );
            return;
        }

        let cmd_str = self.setup.as_ref().unwrap();

        println!(
            "{} {}",
            "  Command:".bright_blue(),
            cmd_str.bright_green().italic()
        );

        let envs = self.get_project_env();

        #[cfg(unix)]
        let mut cmd = std::process::Command::new("sh");
        #[cfg(unix)]
        cmd.arg("-c").arg(cmd_str);

        #[cfg(windows)]
        let mut cmd = std::process::Command::new("cmd");
        #[cfg(windows)]
        cmd.arg("/C").arg(cmd_str);

        cmd.current_dir(&self.root);
        cmd.envs(&envs);
        cmd.stdout(std::process::Stdio::inherit());
        cmd.stderr(std::process::Stdio::inherit());

        match cmd.status() {
            Ok(status) if status.success() => {
                println!(
                    "{} {}\n",
                    " Setup completed successfully in:".bright_green().bold(),
                    self.root.bright_yellow()
                );
            }

            Ok(status) => {
                eprintln!(
                    "{} {} {}",
                    " Setup failed in:".bright_red().bold(),
                    self.root.bright_yellow(),
                    format!("(Exit code: {})", status.code().unwrap_or(-1)).bright_red()
                );
                println!();
            }

            Err(e) => {
                eprintln!(
                    "{} {}: {}",
                    " Failed to execute setup in:".bright_red().bold(),
                    self.root.bright_yellow(),
                    e.to_string().bright_red()
                );
                println!();
            }
        }

        println!(
            "{}",
            "────────────────────────────────────────────".bright_black()
        );
    }
    pub fn get_project_env(&self) -> HashMap<String, String> {
        let mut out = HashMap::new();
        if let Some(env_path) = &self.env_path {
            let path_buf = Path::new(&self.root).join(env_path);
            match load_env_from_file(path_buf) {
                Ok(env) => out = env,
                Err(e) => println!("Error in project {} : {}", self.name, e.to_string()),
            }
        }
        if let Some(envs) = &self.env {
            for (k, v) in envs.iter() {
                out.insert(k.to_string(), v.to_string());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_no_env() {
        let project = Project {
            name: "test".into(),
            root: ".".into(),
            env: None,
            env_path: None,
            setup: None,
            tasks: HashMap::new(),
        };
        let envs = project.get_project_env();
        assert_eq!(envs, HashMap::new());
    }

    #[test]
    fn handle_env_from_file() {
        let project = Project {
            name: "test".into(),
            root: "./".into(),
            env: None,
            env_path: Some(".env.test".to_string()),
            setup: None,
            tasks: HashMap::new(),
        };
        let envs = project.get_project_env();
        assert_eq!(envs.get("APP_NAME").unwrap(), "fyrer-dev");
    }

    #[test]
    fn handle_env_in_config() {
        let mut env_map = HashMap::new();
        env_map.insert("APP_NAME".to_string(), "fyrer-dev".to_string());
        env_map.insert("PORT".to_string(), "3000".to_string());
        let project = Project {
            name: "test".into(),
            root: "./".into(),
            env: Some(env_map),
            env_path: None,
            setup: None,
            tasks: HashMap::new(),
        };
        let envs = project.get_project_env();
        assert_eq!(envs.get("APP_NAME").unwrap(), "fyrer-dev");
        assert_eq!(envs.get("PORT").unwrap(), "3000");
        assert_eq!(envs.get("DOESNOTeXISt"), None)
    }

    #[test]
    fn handle_env_override() {
        let mut env_map = HashMap::new();
        env_map.insert("APP_NAME".to_string(), "overridden".to_string());
        env_map.insert("PORT".to_string(), "3000".to_string());
        let project = Project {
            name: "test".into(),
            root: "./".into(),
            env: Some(env_map),
            env_path: Some(".env.test".into()),
            setup: None,
            tasks: HashMap::new(),
        };

        let envs = project.get_project_env();
        assert_eq!(envs.get("APP_NAME").unwrap(), "overridden");
        assert_eq!(envs.get("DB_HOST").unwrap(), "localhost");
    }
}
