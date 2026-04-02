#[derive(Debug)]
pub enum Log {
    SystemLog {
        log: String,
        project_name: Option<String>,
        // log_type: SystemLogType,
    },
    ProjectLog {
        log: String,
        project_name: String,
        log_type: ProjectLogType,
    },
}

#[derive(Debug)]
pub enum ProjectLogType {
    Stdout,
    Stderr,
}

// #[derive(Debug)]
// pub enum SystemLogType {}
