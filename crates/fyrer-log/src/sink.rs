use anyhow::Result;

use crate::router::LogLine;

pub trait Sink: Send + Sync {
    fn write(&self, line: &LogLine) -> Result<()>;
}

pub struct StdoutSink;
impl Sink for StdoutSink {
    fn write(&self, line: &LogLine) -> Result<()> {
        println!("[{}] {}", line.key.task, line.line);
        Ok(())
    }
}
