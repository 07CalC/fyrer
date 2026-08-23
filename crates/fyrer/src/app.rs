use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use owo_colors::OwoColorize;

use crate::cli::{Cli, Command};
use fyrer_cache::local::LocalCacheProvider;
use fyrer_config::Workspace;
use fyrer_core::{TaskId, spec::TaskRegistry};
use fyrer_engine::{
    events::{RunPlan, RunSummary},
    handle::EngineBuilder,
};
use fyrer_log::LogRouter;
use fyrer_ui::reporter::Reporter;
use fyrer_ui::{plain::PlainReporter, tui::Tui};
use fyrer_watch::Watcher;

pub struct App {
    cli: Cli,
}

impl App {
    pub fn new() -> Self {
        let cli = Cli::parse();
        Self { cli }
    }

    pub async fn run(&self) -> Result<()> {
        let config_path = &self.cli.config;
        let command = &self.cli.command;
        let workspace = Workspace::new_from_path(config_path)?;
        workspace.validate()?;
        match command {
            Command::Run { task, no_tui } => {
                self.run_tasks(workspace, task.as_deref(), *no_tui).await?;
            }
            Command::Plan { task } => {
                self.plan(&workspace, task.as_deref())?;
            }
            Command::List => {
                workspace.list_tasks();
            }
        }
        Ok(())
    }

    fn resolve_task_ids(
        &self,
        registry: &TaskRegistry,
        spec: Option<&str>,
    ) -> Result<Vec<TaskId>> {
        match spec {
            Some(s) if s.contains(':') => {
                let id = TaskId::from_str(s)
                    .ok_or_else(|| anyhow::anyhow!("Invalid task specifier: {}", s))?;
                if registry.get(&id).is_some() {
                    Ok(vec![id])
                } else {
                    Err(anyhow::anyhow!("Task {} not found", id))
                }
            }
            Some(s) => {
                let ids: Vec<TaskId> = registry
                    .iter()
                    .filter(|(tid, _)| tid.task() == s)
                    .map(|(tid, _)| tid.clone())
                    .collect();
                if ids.is_empty() {
                    Err(anyhow::anyhow!("Task {} not found", s))
                } else {
                    Ok(ids)
                }
            }
            None => Ok(registry.iter().map(|(tid, _)| tid.clone()).collect()),
        }
    }

    fn plan(&self, workspace: &Workspace, spec: Option<&str>) -> Result<()> {
        let graph = workspace
            .task_graph()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let registry = workspace.task_registry();
        let task_ids = self.resolve_task_ids(&registry, spec)?;
        if task_ids.is_empty() {
            return Err(anyhow::anyhow!("No tasks found for the given specifier"));
        }
        let levels = graph
            .get_levels(&task_ids)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        println!("\nExecution plan:");
        for (i, level) in levels.iter().enumerate() {
            println!("  Level {} (parallel):", i + 1);
            for (j, task_id) in level.iter().enumerate() {
                let prefix = if j + 1 == level.len() { "└──" } else { "├──" };
                println!("    {prefix} {task_id}");
            }
            if i + 1 < levels.len() {
                println!("         ↓");
            }
        }
        println!();
        Ok(())
    }

    async fn run_tasks(
        &self,
        workspace: Workspace,
        spec: Option<&str>,
        no_tui: bool,
    ) -> Result<()> {
        let graph = workspace
            .task_graph()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let registry = workspace.task_registry();
        let task_ids = self.resolve_task_ids(&registry, spec)?;
        if task_ids.is_empty() {
            return Err(anyhow::anyhow!("No tasks found for the given specifier"));
        }

        let has_watch = registry.iter().any(|(_, s)| s.watch);
        let cache: Arc<dyn fyrer_cache::provider::CacheProvider> =
            Arc::new(LocalCacheProvider::new(".fyrer/cache".to_string()));
        let log_router = Arc::new(LogRouter::new(500, None));

        // All runs go through EngineBuilder::spawn so reporters get a control
        // channel (restart/kill keybinds) and the engine stays addressable.
        let mut builder =
            EngineBuilder::new(registry.clone(), graph.clone(), cache.clone())
                .log_router(log_router.clone())
                .interactive(!no_tui || has_watch);
        if let Some(c) = workspace.concurrency {
            builder = builder.concurrency(c);
        }
        let handle = builder.spawn(RunPlan::new(task_ids.clone()));

        // Start reporter — TUI gets the command channel for restart/kill.
        let reporter_handle = if no_tui {
            PlainReporter::default().start(handle.subscribe())
        } else {
            Tui::new().start_with_control(handle.subscribe(), Some(handle.cmd_sender()))
        };

        // File watcher for watch-mode tasks.
        let watcher_handle = if has_watch {
            let watcher = Watcher::new(registry.clone());
            Some(watcher.spawn(handle.cmd_sender()))
        } else {
            None
        };

        // Wait for the engine: one-shot runs finish on their own; watch runs
        // stay alive until the TUI quit / Ctrl+C triggers Shutdown.
        let summary = handle.wait().await?;
        if let Some(w) = watcher_handle {
            w.abort();
        }

        // Plain UI exits on RunFinished; TUI exits when the user quits, so no
        // timeout is needed there (terminal must be restored before printing).
        if no_tui {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), reporter_handle).await;
        } else {
            let _ = reporter_handle.await;
        }

        self.print_summary(&summary);
        if summary.failed > 0 {
            std::process::exit(1);
        }
        Ok(())
    }

    fn print_summary(&self, summary: &RunSummary) {
        println!();
        println!(
            "{} {}",
            "Run completed in".bold(),
            format!("{:.2?}", summary.duration).dimmed()
        );
        println!();
        println!("  {}", "Results".bold());
        println!("  {}", "─────────────────────────".dimmed());
        println!(
            "  {} {:<12} {}",
            "+".green().bold(),
            "Successful",
            summary.successful.to_string().green()
        );
        println!(
            "  {} {:<12} {}",
            "x".red().bold(),
            "Failed",
            summary.failed.to_string().red()
        );
        println!(
            "  {} {:<12} {}",
            "*".cyan().bold(),
            "Cached",
            summary.cached.to_string().cyan()
        );
        println!(
            "  {} {:<12} {}",
            "-".yellow().bold(),
            "Skipped",
            summary.skipped.to_string().yellow()
        );
        println!("  {:<14} {}", "Total".bold(), summary.total.to_string().bold());
        if summary.cached == summary.total && summary.total > 0 {
            println!();
            println!("  {}", "ALL CACHED".cyan().bold());
            println!("  {}", "FYRER FIRED.".cyan().bold());
            println!();
        }
    }
}
