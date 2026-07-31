use std::path::{Path, PathBuf};
use std::time::Duration;

use glob::MatchOptions;
use notify::{Event, RecursiveMode, Watcher};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

use crate::{
    config::RestartStrategy,
    error::{FyrerError, FyrerResult, watch::WatcherError},
    executor::{TaskProcess, start_task},
    global,
    logger::{LogMessage, LogType},
    tasks::{Task, TaskId},
};

const DEFAULT_DEBOUNCE_MS: u64 = 200;

const MATCH_OPTIONS: MatchOptions = MatchOptions {
    case_sensitive: true,
    require_literal_separator: true,
    require_literal_leading_dot: true,
};

pub async fn watch_tasks(running: Vec<(Task, TaskProcess)>) -> FyrerResult<()> {
    if running.is_empty() {
        return Ok(());
    }
    for (task, process) in running {
        setup_watch(task, process).await?;
    }
    std::future::pending().await
}

async fn setup_watch(task: Task, process: TaskProcess) -> FyrerResult<()> {
    if task.restart.strategy != RestartStrategy::FileChange {
        return Ok(());
    }
    let root = std::path::absolute(&task.project_root).map_err(|e| {
        FyrerError::Watch(WatcherError::ResolveRoot {
            path: task.project_root.display().to_string(),
            source: e,
        })
    })?;
    if !root.is_dir() {
        return Err(FyrerError::Watch(WatcherError::MissingRoot(
            root.display().to_string(),
        )));
    }

    let matcher = TaskMatcher::new(&task, &root);
    let (sender, receiver) = unbounded_channel();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
        if let Ok(event) = res {
            let _ = sender.send(event);
        }
    })
    .map_err(|e| FyrerError::Watch(WatcherError::Init(e)))?;
    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|e| FyrerError::Watch(WatcherError::Init(e)))?;

    let task_id = TaskId::new(&task.project_name, &task.task_name);
    global::get()
        .log_sender
        .send(LogMessage {
            task_id: task_id.clone(),
            message: "watching for changes".to_string(),
            log_type: LogType::System,
        })
        .await
        .map_err(|e| FyrerError::Watch(WatcherError::LogSend(e)))?;
    tokio::spawn(watch_loop(task, matcher, watcher, receiver, process));
    Ok(())
}

async fn watch_loop(
    task: Task,
    matcher: TaskMatcher,
    _watcher: notify::RecommendedWatcher,
    mut events: UnboundedReceiver<Event>,
    mut process: TaskProcess,
) {
    let task_id = TaskId::new(&task.project_name, &task.task_name);
    let debounce = Duration::from_millis(task.restart.delay.unwrap_or(DEFAULT_DEBOUNCE_MS));

    loop {
        let Some(event) = events.recv().await else {
            return;
        };
        if !matcher.matches(&event) {
            continue;
        }

        loop {
            tokio::select! {
                _ = tokio::time::sleep(debounce) => break,
                Some(next) = events.recv() => {
                    if matcher.matches(&next) {
                    }
                }
            }
        }

        let _ = global::get()
            .log_sender
            .send(LogMessage {
                task_id: task_id.clone(),
                message: "change detected, restarting".to_string(),
                log_type: LogType::System,
            })
            .await;

        process.stop().await;
        if global::is_shutting_down() {
            return;
        }
        match start_task(task.clone()).await {
            Ok(next) => process = next,
            Err(e) => eprintln!("error: failed to restart {task_id}: {e}"),
        }
    }
}

struct TaskMatcher {
    inputs: Vec<glob::Pattern>,
    ignore: Vec<glob::Pattern>,
    root: PathBuf,
}

impl TaskMatcher {
    fn new(task: &Task, root: &Path) -> Self {
        let inputs = task
            .inputs
            .iter()
            .filter_map(|pattern| glob::Pattern::new(pattern).ok())
            .collect();
        let ignore = task
            .ignore
            .iter()
            .filter_map(|pattern| glob::Pattern::new(pattern).ok())
            .collect();
        TaskMatcher {
            inputs,
            ignore,
            root: std::path::absolute(root).unwrap_or_else(|_| root.to_path_buf()),
        }
    }

    fn matches(&self, event: &Event) -> bool {
        if !(event.kind.is_create() || event.kind.is_modify() || event.kind.is_remove()) {
            return false;
        }
        event.paths.iter().any(|path| self.is_relevant(path))
    }

    fn is_relevant(&self, path: &Path) -> bool {
        let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        let relative = match path.strip_prefix(&self.root) {
            Ok(relative) => relative,
            Err(_) => return false,
        };
        let is_input = self
            .inputs
            .iter()
            .any(|pattern| pattern.matches_path_with(relative, MATCH_OPTIONS));
        let is_ignored = self
            .ignore
            .iter()
            .any(|pattern| pattern.matches_path_with(relative, MATCH_OPTIONS));
        is_input && !is_ignored
    }
}

#[cfg(test)]
mod tests {
    use super::TaskMatcher;
    use crate::{
        config::{RestartConfig, RestartStrategy},
        tasks::Task,
    };
    use notify::event::ModifyKind;
    use notify::{Event, EventKind};
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    fn task_with(root: &str, inputs: &[&str], ignore: &[&str]) -> Task {
        Task {
            project_name: "web".to_string(),
            project_root: PathBuf::from(root),
            env: HashMap::new(),
            task_name: "dev".to_string(),
            cmd: "echo hi".to_string(),
            depends_on: vec![],
            inputs: inputs.iter().map(|s| s.to_string()).collect(),
            outputs: vec![],
            ignore: ignore.iter().map(|s| s.to_string()).collect(),
            cache: false,
            restart: RestartConfig {
                strategy: RestartStrategy::FileChange,
                delay: Some(100),
            },
        }
    }

    fn modify_event(path: PathBuf) -> Event {
        Event::new(EventKind::Modify(ModifyKind::Data(
            notify::event::DataChange::Any,
        )))
        .add_path(path)
    }

    #[test]
    fn test_matches_relevant_and_ignored_paths() {
        let matcher = TaskMatcher::new(
            &task_with(
                ".",
                &["src/**/*", "package.json"],
                &["node_modules/**", "dist/**"],
            ),
            Path::new("."),
        );

        assert!(matcher.matches(&modify_event(PathBuf::from("src/main.rs"))));
        assert!(matcher.matches(&modify_event(PathBuf::from("package.json"))));
        assert!(matcher.matches(&modify_event(PathBuf::from("src/lib/helper.rs"))));
        assert!(!matcher.matches(&modify_event(PathBuf::from("node_modules/pkg/index.js"))));
        assert!(!matcher.matches(&modify_event(PathBuf::from("dist/bundle.js"))));
        assert!(!matcher.matches(&modify_event(PathBuf::from("README.md"))));
        assert!(!matcher.matches(&modify_event(PathBuf::from("src/.main.rs.swp"))));
    }

    #[test]
    fn test_ignore_overrides_input() {
        let matcher = TaskMatcher::new(&task_with(".", &["**/*"], &["*.log"]), Path::new("."));
        assert!(matcher.matches(&modify_event(PathBuf::from("anything.txt"))));
        assert!(!matcher.matches(&modify_event(PathBuf::from("server.log"))));
    }

    #[test]
    fn test_ignores_non_modifying_events() {
        let matcher = TaskMatcher::new(&task_with(".", &["**/*"], &[]), Path::new("."));
        let access = Event::new(EventKind::Access(notify::event::AccessKind::Any))
            .add_path(PathBuf::from("src/main.rs"));
        assert!(!matcher.matches(&access));
    }

    #[test]
    fn test_path_outside_root_is_irrelevant() {
        let matcher = TaskMatcher::new(&task_with(".", &["**/*"], &[]), Path::new("."));
        assert!(!matcher.matches(&modify_event(PathBuf::from("/etc/hosts"))));
    }

    #[test]
    fn test_glob_star_does_not_cross_directories() {
        let matcher = TaskMatcher::new(&task_with(".", &["*.rs"], &[]), Path::new("."));
        assert!(matcher.matches(&modify_event(PathBuf::from("main.rs"))));
        assert!(!matcher.matches(&modify_event(PathBuf::from("src/main.rs"))));
    }
}
