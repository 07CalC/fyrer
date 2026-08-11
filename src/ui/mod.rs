use anyhow::Result;

use crate::{
    events::LogStream,
    task::{TaskId, TaskStatus},
};

pub mod plain;
pub mod tui;
pub trait Ui {
    fn push_log(&mut self, task_id: &TaskId, line: String, stream: LogStream);
    fn render(&mut self, tasks: &[(TaskId, TaskStatus)]) -> Result<()>;
    fn navigate_next(&mut self) {}
    fn navigate_previous(&mut self) {}
    fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
    fn scroll_logs_up(&mut self) {}
    fn scroll_logs_down(&mut self) {}
    fn scroll_logs_up_by(&mut self, _n: usize) {}
    fn scroll_logs_down_by(&mut self, _n: usize) {}
}
