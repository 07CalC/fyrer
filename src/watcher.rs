use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use glob::MatchOptions;
use notify::{Event, RecursiveMode, Watcher};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

use crate::{
    config::RestartStrategy,
    error::{FyrerError, FyrerResult, WatcherError},
    executor::{RunningTask, TaskProcess, start_task},
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

/// Starts a file watcher for each long-running task and then waits forever
/// (until a shutdown signal arrives).
///
/// # Errors
///
/// Returns an error if a project root cannot be resolved or watched, or if a
/// log message cannot be sent.
pub async fn watch_tasks(running: Vec<RunningTask>) -> FyrerResult<()> {
    if running.is_empty() {
        return Ok(());
    }
    for entry in running {
        setup_watch(entry.task, entry.process).await?;
    }
    std::future::pending().await
}

async fn setup_watch(task: Task, process: TaskProcess) -> FyrerResult<()> {
    if task.restart.strategy != RestartStrategy::FileChange {
        return Ok(());
    }
    let root = std::path::absolute(&task.project_root).map_err(|source| {
        FyrerError::Watch(WatcherError::ResolveRoot {
            path: task.project_root.display().to_string(),
            source,
        })
    })?;
    if !root.is_dir() {
        return Err(FyrerError::Watch(WatcherError::MissingRoot(
            root.display().to_string(),
        )));
    }

    let matcher = TaskMatcher::new(&task, &root);
    let (sender, receiver) = unbounded_channel();
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<Event>| {
        if let Ok(event) = result {
            let _ = sender.send(event);
        }
    })
    .map_err(WatcherError::Init)?;
    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(WatcherError::Init)?;

    let task_id = TaskId::new(&task.project_name, &task.task_name);
    global::get()
        .log_sender
        .send(LogMessage {
            task_id: task_id.clone(),
            message: "watching for changes".to_string(),
            log_type: LogType::System,
        })
        .await
        .map_err(WatcherError::LogSend)?;
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

        // Debounce: keep draining events until no change arrives for
        // `debounce`, so a burst of changes triggers a single restart.
        loop {
            tokio::select! {
                () = tokio::time::sleep(debounce) => break,
                Some(_) = events.recv() => {}
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
            Err(error) => eprintln!("error: failed to restart {task_id}: {error}"),
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
        Self {
            inputs: task
                .inputs
                .iter()
                .filter_map(|pattern| glob::Pattern::new(pattern).ok())
                .collect(),
            ignore: task
                .ignore
                .iter()
                .filter_map(|pattern| glob::Pattern::new(pattern).ok())
                .collect(),
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
        let Ok(path) = std::path::absolute(path) else {
            return false;
        };
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return false;
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
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    use notify::{Event, EventKind, event::ModifyKind};

    use super::TaskMatcher;
    use crate::{
        config::{RestartConfig, RestartStrategy},
        tasks::Task,
    };

    fn task_with(root: &str, inputs: &[&str], ignore: &[&str]) -> Task {
        Task {
            project_name: "web".to_string(),
            project_root: PathBuf::from(root),
            env: HashMap::new(),
            task_name: "dev".to_string(),
            cmd: "echo hi".to_string(),
            depends_on: vec![],
            inputs: inputs.iter().map(ToString::to_string).collect(),
            outputs: vec![],
            ignore: ignore.iter().map(ToString::to_string).collect(),
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
    fn matches_relevant_and_ignored_paths() {
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
    fn ignore_overrides_input() {
        let matcher = TaskMatcher::new(&task_with(".", &["**/*"], &["*.log"]), Path::new("."));
        assert!(matcher.matches(&modify_event(PathBuf::from("anything.txt"))));
        assert!(!matcher.matches(&modify_event(PathBuf::from("server.log"))));
    }

    #[test]
    fn ignores_non_modifying_events() {
        let matcher = TaskMatcher::new(&task_with(".", &["**/*"], &[]), Path::new("."));
        let access = Event::new(EventKind::Access(notify::event::AccessKind::Any))
            .add_path(PathBuf::from("src/main.rs"));
        assert!(!matcher.matches(&access));
    }

    #[test]
    fn path_outside_root_is_irrelevant() {
        let matcher = TaskMatcher::new(&task_with(".", &["**/*"], &[]), Path::new("."));
        assert!(!matcher.matches(&modify_event(PathBuf::from("/etc/hosts"))));
    }

    #[test]
    fn glob_star_does_not_cross_directories() {
        let matcher = TaskMatcher::new(&task_with(".", &["*.rs"], &[]), Path::new("."));
        assert!(matcher.matches(&modify_event(PathBuf::from("main.rs"))));
        assert!(!matcher.matches(&modify_event(PathBuf::from("src/main.rs"))));
    }
}
