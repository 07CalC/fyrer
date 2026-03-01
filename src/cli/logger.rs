use colored::Colorize;
use tokio::{
    sync::mpsc::{self, Sender},
    task,
};

use crate::cli::print_banner::print_banner;

#[derive(Debug)]
pub enum LogKind {
    Output,
    Error,
    System,
}

#[derive(Debug)]
pub struct Log {
    pub service: String,
    pub message: String,
    pub kind: LogKind,
    pub color: colored::Color,
}

pub struct Logger {
    tx: Sender<Log>,
}

impl Logger {
    pub fn new(limit: usize, max_name_len: usize) -> Logger {
        let (tx, mut rx) = mpsc::channel::<Log>(limit);
        task::spawn(async move {
            while let Some(log) = rx.recv().await {
                let prefix = format!("[{}]", log.service);
                let padded = format!("{:width$}", prefix, width = max_name_len);
                match log.kind {
                    LogKind::Output => {
                        println!(
                            "├─{} ➤ {}",
                            padded.color(log.color).bold(),
                            log.message.color(log.color)
                        );
                    }
                    LogKind::Error => {
                        eprintln!(
                            "├─{} ⚠ {}",
                            padded.color(log.color).bold(),
                            log.message.bright_red()
                        );
                    }
                    LogKind::System => {
                        println!(
                            "├─{} ➤ {}",
                            padded.color(log.color).bold(),
                            log.message.bright_white()
                        );
                    }
                }
            }
        });
        Self { tx }
    }
    pub fn sender(&self) -> Sender<Log> {
        self.tx.clone()
    }
}
