use colored::{Color, Colorize};
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::tasks::TaskId;

const COLORS: [Color; 10] = [
    Color::Green,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::Cyan,
    Color::BrightYellow,
    Color::BrightMagenta,
    Color::BrightCyan,
    Color::AnsiColor(208),
    Color::AnsiColor(214),
];

#[derive(Debug, Clone, Copy)]
pub enum LogType {
    Info,
    Error,
    Warning,
    System,
}

#[derive(Debug)]
pub struct LogMessage {
    pub task_id: TaskId,
    pub message: String,
    pub log_type: LogType,
}

#[derive(Debug)]
pub struct Logger {
    /// Clones of this sender will be used to push messages to the logger.
    pub sender: Sender<LogMessage>,
    receiver: Receiver<LogMessage>,
}

impl Logger {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = mpsc::channel(capacity);
        Self { sender, receiver }
    }

    #[must_use]
    pub fn sender(&self) -> Sender<LogMessage> {
        self.sender.clone()
    }

    pub async fn start(&mut self) {
        while let Some(message) = self.receiver.recv().await {
            let color = COLORS[message.task_id.hash() % COLORS.len()];
            let header = format!("[{}]", message.task_id).color(color);
            let body = match message.log_type {
                LogType::Error => message.message.red().to_string(),
                _ => message.message,
            };
            println!("{header}: {body}");
        }
    }
}
