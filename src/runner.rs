use crate::config::Project;
use crate::spawn_service::spawn_service;
use crate::watcher::run_with_watch;

pub async fn runner(service: Project, color: colored::Color, max_name_len: usize) {
    if service.watch.unwrap_or(false) {
        run_with_watch(service, color, max_name_len).await;
    } else {
        spawn_service(&service, color, true, max_name_len).await;
    }
}
