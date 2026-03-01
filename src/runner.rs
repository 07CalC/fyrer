use tokio::sync::mpsc::Sender;

use crate::cli::logger::Log;
use crate::config::Service;
use crate::spawn_service::spawn_service;
use crate::watcher::run_with_watch;

pub async fn runner(service: Service, color: colored::Color, sender: Sender<Log>) {
    if service.watch.unwrap_or(false) {
        run_with_watch(service, color, sender).await;
    } else {
        spawn_service(&service, color, true, sender).await;
    }
}
