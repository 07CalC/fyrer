use crate::task::TaskId;

#[derive(Debug, Clone)]
pub enum ProcessLog {
    Stdout { task_id: TaskId, data: Vec<u8> },
    Stderr { task_id: TaskId, data: Vec<u8> },
    System { task_id: TaskId, data: Vec<u8> },
}
