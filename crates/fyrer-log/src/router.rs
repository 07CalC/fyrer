use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use fyrer_core::{ExecKey, TaskId};
use tokio::sync::mpsc;

use crate::buffer::RingBuffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStream {
    Stdout,
    Stderr,
    System,
}

#[derive(Debug, Clone)]
pub struct LogLine {
    pub key: ExecKey,
    pub stream: LogStream,
    pub line: String,
}

// Simple router: fans out to ring buffers and optional sinks.
// For now, also holds transcripts on disk under .fyrer/logs/<run>/<task>.<attempt>.log if desired.

pub struct LogRouter {
    buffers: Arc<Mutex<HashMap<TaskId, RingBuffer<LogLine>>>>,
    tx: mpsc::Sender<LogLine>,
    _handle: tokio::task::JoinHandle<()>,
}

impl LogRouter {
    pub fn new(capacity_per_task: usize, transcript_dir: Option<PathBuf>) -> Self {
        let buffers: Arc<Mutex<HashMap<TaskId, RingBuffer<LogLine>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let buffers_clone = Arc::clone(&buffers);
        let (tx, mut rx) = mpsc::channel::<LogLine>(1024);

        let handle = tokio::spawn(async move {
            while let Some(line) = rx.recv().await {
                // ring buffer
                {
                    let mut map = buffers_clone.lock().unwrap();
                    let buf = map
                        .entry(line.key.task.clone())
                        .or_insert_with(|| RingBuffer::new(capacity_per_task));
                    buf.push(line.clone());
                }
                // transcript
                if let Some(dir) = transcript_dir.as_ref() {
                    let _ = write_transcript(dir, &line);
                }
            }
        });

        Self {
            buffers,
            tx,
            _handle: handle,
        }
    }

    pub fn sender(&self) -> mpsc::Sender<LogLine> {
        self.tx.clone()
    }

    pub fn buffer_for(&self, task: &TaskId) -> Vec<LogLine> {
        self.buffers
            .lock()
            .unwrap()
            .get(task)
            .map(|b| b.to_vec())
            .unwrap_or_default()
    }

    pub async fn push(&self, line: LogLine) {
        let _ = self.tx.send(line).await;
    }
}

fn write_transcript(dir: &PathBuf, line: &LogLine) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.{}.log", line.key.task, line.key.attempt.0));
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(f, "{}", line.line)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ring_basic() {
        let mut rb = RingBuffer::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        assert_eq!(rb.to_vec(), vec![2, 3]);
    }
}
