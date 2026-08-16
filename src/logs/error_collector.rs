use crate::task::TaskId;
use owo_colors::OwoColorize;

pub enum Diagnostics {
    Error {
        task_id: Option<TaskId>,
        error: String,
    },
    Warning {
        task_id: Option<TaskId>,
        message: String,
    },
}
pub struct ErrorCollector {
    errors: Vec<Diagnostics>,
}

impl ErrorCollector {
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    pub fn push_error(&mut self, task_id: Option<TaskId>, error: String) {
        self.errors.push(Diagnostics::Error { task_id, error });
    }

    pub fn push_warning(&mut self, task_id: Option<TaskId>, message: String) {
        self.errors.push(Diagnostics::Warning { task_id, message });
    }

    #[allow(dead_code)]
    fn push(&mut self, diagnostic: Diagnostics) {
        self.errors.push(diagnostic);
    }
    pub fn get_errors(&self) -> &[Diagnostics] {
        &self.errors
    }

    pub fn finalize(&self) {
        if self.errors.is_empty() {
            return;
        }

        let mut error_count = 0;
        let mut warning_count = 0;

        println!();
        println!("  {}", "Diagnostics".bold());
        println!("  {}", "─────────────────────────".dimmed());

        for diagnostic in &self.errors {
            match diagnostic {
                Diagnostics::Error { task_id, error } => {
                    error_count += 1;

                    if let Some(task_id) = task_id {
                        eprintln!("  {} {}", "x".red().bold(), task_id.bold());
                        eprintln!("    {}", error);
                    } else {
                        eprintln!("  {} {}", "x".red().bold(), error);
                    }

                    eprintln!();
                }

                Diagnostics::Warning { task_id, message } => {
                    warning_count += 1;

                    if let Some(task_id) = task_id {
                        eprintln!("  {} {}", "!".yellow().bold(), task_id.bold());
                        eprintln!("    {}", message);
                    } else {
                        eprintln!("  {} {}", "!".yellow().bold(), message);
                    }

                    eprintln!();
                }
            }
        }

        let mut summary = Vec::new();

        if error_count > 0 {
            summary.push(format!(
                "{} {}",
                error_count,
                if error_count == 1 { "error" } else { "errors" }
            ));
        }

        if warning_count > 0 {
            summary.push(format!(
                "{} {}",
                warning_count,
                if warning_count == 1 {
                    "warning"
                } else {
                    "warnings"
                }
            ));
        }

        println!("  {}", summary.join(" · ").dimmed());
    }
}
