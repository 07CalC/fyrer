use crate::tasks::TaskId;
use tokio::sync::mpsc::{self, Receiver, Sender};

static COLORS: [&str; 10] = [
    "\x1b[32m",       // Green
    "\x1b[33m",       // Yellow
    "\x1b[34m",       // Blue
    "\x1b[35m",       // Magenta
    "\x1b[36m",       // Cyan
    "\x1b[93m",       // Bright Yellow
    "\x1b[95m",       // Bright Magenta
    "\x1b[96m",       // Bright Cyan
    "\x1b[38;5;208m", // Orange
    "\x1b[38;5;214m", // Light Orange
];

#[derive(Debug)]
pub struct LogMessage {
    pub task_id: TaskId,
    pub message: String,
    pub log_type: LogType,
}
#[derive(Debug)]
pub enum LogType {
    Info,
    Error,
    Warning,
    System,
}

#[derive(Debug)]
pub struct Logger {
    pub sender: Sender<LogMessage>,
    pub receiver: Receiver<LogMessage>,
}

impl Logger {
    pub fn new(size: usize) -> Self {
        let (tx, rx) = mpsc::channel(size);
        Logger {
            sender: tx,
            receiver: rx,
        }
    }
    pub fn add_task(&self) -> Sender<LogMessage> {
        self.sender.clone()
    }
    pub async fn start(&mut self) {
        while let Some(log_message) = self.receiver.recv().await {
            let color_index = log_message.task_id.hash() % COLORS.len();
            let color = COLORS[color_index];
            println!(
                "{}[{}]:{} {}{}{}",
                color,
                log_message.task_id.to_string(),
                "\x1b[0m",
                if let LogType::Error = log_message.log_type {
                    "\x1b[31m"
                } else {
                    ""
                },
                log_message.message,
                "\x1b[0m"
            );
        }
    }
}
