use clap::{Parser, Subcommand};
use fyrer::config::FyrerConfig;
use fyrer::error::{FyrerError, FyrerResult, graph::GraphError};
use fyrer::executor;
use fyrer::global::{self};
use fyrer::graph::TaskGraph;
use fyrer::logger::Logger;
use fyrer::tasks::{TaskId, TaskMap};
use std::collections::BTreeMap;

#[derive(Parser)]
#[command(
    name = "fyrer",
    version,
    about = "A declarative, fast and lightweight monorepo task runner"
)]
struct Cli {
    #[arg(short, long, default_value = "fyrer.yml")]
    config: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Run {
        task: Option<String>,
        #[arg(long)]
        dry_run: bool,
    },
    List,
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}

async fn run() -> FyrerResult<()> {
    let cli = Cli::parse();
    let config = FyrerConfig::new_from_path(&cli.config)?;
    let task_map = config.create_task_map();

    match cli.command {
        Command::List => list(&task_map),
        Command::Run { task, dry_run } => {
            let task_graph = TaskGraph::new(&task_map)?;
            task_graph.validate()?;
            let tasks = resolve_tasks(&task_map, task.as_deref())?;
            if dry_run {
                return print_plan(&task_graph, &tasks);
            }
            let mut logger = Logger::new(task_map.len());
            let log_sender = logger.add_task();
            tokio::spawn(async move {
                logger.start().await;
            });
            global::init(task_graph, task_map, config.env, log_sender)?;
            executor::execute_tasks(&tasks).await
        }
    }
}

fn list(task_map: &TaskMap) -> FyrerResult<()> {
    let mut projects: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for id in task_map.keys() {
        projects
            .entry(id.project_name())
            .or_default()
            .push(id.task_name());
    }
    for (project, mut tasks) in projects {
        tasks.sort_unstable();
        println!("{project}");
        for task in tasks {
            println!("  {task}");
        }
    }
    Ok(())
}

fn resolve_tasks(task_map: &TaskMap, spec: Option<&str>) -> FyrerResult<Vec<TaskId>> {
    match spec {
        None => {
            let mut all: Vec<TaskId> = task_map.keys().cloned().collect();
            all.sort_by_key(|id| id.to_string());
            Ok(all)
        }
        Some(spec) if spec.contains(':') => {
            let id = TaskId::from_string(spec).ok_or_else(|| {
                FyrerError::Graph(GraphError::InvalidTaskId {
                    dependency: spec.to_string(),
                    task: spec.to_string(),
                })
            })?;
            if !task_map.contains_key(&id) {
                return Err(FyrerError::Graph(GraphError::TaskNotFound(
                    spec.to_string(),
                )));
            }
            Ok(vec![id])
        }
        Some(spec) => {
            let mut matches: Vec<TaskId> = task_map
                .keys()
                .filter(|id| id.task_name() == spec)
                .cloned()
                .collect();
            matches.sort_by_key(|id| id.to_string());
            if matches.is_empty() {
                return Err(FyrerError::Graph(GraphError::TaskNotFound(
                    spec.to_string(),
                )));
            }
            Ok(matches)
        }
    }
}

fn print_plan(task_graph: &TaskGraph, tasks: &[TaskId]) -> FyrerResult<()> {
    let levels = task_graph.get_orders(tasks)?;
    for (i, level) in levels.iter().enumerate() {
        let names: Vec<String> = level.iter().map(|id| id.to_string()).collect();
        println!("step {}: {}", i + 1, names.join(", "));
    }
    Ok(())
}
