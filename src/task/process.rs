use crate::events::TaskCommand;
use tokio::{sync::mpsc::Sender, task::JoinHandle};

#[derive(Debug)]
pub struct TaskProcess {
    pub handle: JoinHandle<bool>,
    pub command_tx: Sender<TaskCommand>,
}
