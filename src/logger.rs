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
    pub sender: Sender<LogMessage>,
    pub receiver: Receiver<LogMessage>,
}

impl Logger {
    pub fn new(size: usize) -> Self {
        let (sender, receiver) = mpsc::channel(size);
        Logger { sender, receiver }
    }

    pub fn sender(&self) -> Sender<LogMessage> {
        self.sender.clone()
    }

    pub async fn start(&mut self) {
        while let Some(log_message) = self.receiver.recv().await {
            let color = COLORS[log_message.task_id.hash() % COLORS.len()];
            let task = format!("[{}]", log_message.task_id).color(color);
            let message = match log_message.log_type {
                LogType::Error => log_message.message.red().to_string(),
                _ => log_message.message,
            };
            println!("{task}: {message}");
        }
    }

    pub async fn send(&self, log_message: LogMessage) {
        if let Err(e) = self.sender.send(log_message).await {
            eprintln!("Logger error: {e}");
        }
    }
}
