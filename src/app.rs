use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::cli::{Cli, Command};
use fyrer_cache::local::LocalCacheProvider;
use fyrer_config::workspace::Workspace;
use fyrer_core::{TaskId, spec::TaskRegistry};
use fyrer_engine::{
    Engine,
    events::{EngineEvent, RunPlan},
};
use fyrer_log::LogRouter;
use fyrer_ui::{plain::PlainReporter, tui::Tui};
use fyrer_ui::reporter::Reporter;

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

        // For watch mode we need a persistent engine handle so file changes can trigger restarts.
        // For normal runs we use the one-shot path (fast, no extra wait).
        if has_watch {
            use fyrer_engine::handle::EngineBuilder;
            use fyrer_watch::Watcher;
            let builder = EngineBuilder::new(registry.clone(), graph.clone(), cache.clone())
                .log_router(log_router.clone());
            let builder = if let Some(c) = workspace.concurrency {
                builder.concurrency(c)
            } else {
                builder
            };
            let run_plan = RunPlan::new(task_ids.clone());
            let handle = builder.spawn(run_plan);
            let reporter_handle = if no_tui {
                let rx = handle.subscribe();
                PlainReporter::default().start(rx)
            } else {
                let rx = handle.subscribe();
                Tui::new().start(rx)
            };
            // Start file watcher
            let watcher = Watcher::new(registry.clone());
            let watcher_handle = watcher.spawn(handle.subscribe_cmd_tx());

            // Wait for engine to finish (watch keeps it alive until Ctrl+C)
            // For watch tasks, engine stays alive because `watch` flag makes wait_after_done infinite.
            // Here we just await handle; Ctrl+C inside engine will trigger shutdown.
            let summary = handle.wait().await?;
            watcher_handle.abort();
            let _ = tokio::time::timeout(std::time::Duration::from_secs(1), reporter_handle).await;

            use owo_colors::OwoColorize;
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
            if summary.failed > 0 {
                std::process::exit(1);
            }
            return Ok(());
        }

        let (event_tx, _) = broadcast::channel::<EngineEvent>(2048);
        let run_plan = RunPlan::new(task_ids);

        // Start reporter (UI) — subscribes to the same broadcast
        let reporter_handle = if no_tui {
            let rx = event_tx.subscribe();
            let reporter = PlainReporter::default();
            reporter.start(rx)
        } else {
            let rx = event_tx.subscribe();
            let reporter = Tui::new();
            reporter.start(rx)
        };

        let engine = Engine::new(
            registry,
            graph,
            cache,
            log_router,
            event_tx.clone(),
            workspace.concurrency,
        );

        let summary = engine.run_once(run_plan).await?;

        // Give reporter a moment to drain RunFinished
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), reporter_handle).await;

        // Summary (mirrors old orchestrator output)
        use owo_colors::OwoColorize;
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

        if summary.failed > 0 {
            std::process::exit(1);
        }
        Ok(())
    }
}
