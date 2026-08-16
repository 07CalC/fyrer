use crate::task::TaskId;

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

    fn push(&mut self, diagnostic: Diagnostics) {
        self.errors.push(diagnostic);
    }
    pub fn get_errors(&self) -> &[Diagnostics] {
        &self.errors
    }

    pub fn finalize(&mut self) {
        println!("Following errors and warnings were collected during execution:");
        for diagnostic in &self.errors {
            match diagnostic {
                Diagnostics::Error { task_id, error } => {
                    if let Some(task_id) = task_id {
                        eprintln!("Error in task {}: {}", task_id, error);
                    } else {
                        eprintln!("Error: {}", error);
                    }
                }
                Diagnostics::Warning { task_id, message } => {
                    if let Some(task_id) = task_id {
                        eprintln!("Warning in task {}: {}", task_id, message);
                    } else {
                        eprintln!("Warning: {}", message);
                    }
                }
            }
        }
    }
}
