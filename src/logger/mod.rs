use crate::logger::log::Log;

pub(crate) mod log;

pub struct Logger {
    rx: tokio::sync::mpsc::Receiver<Log>,
    tx: tokio::sync::mpsc::Sender<Log>,
}

impl Logger {
    pub fn new(size: usize) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(size);
        Self { rx, tx }
    }
}
