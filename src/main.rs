use clap::{Parser, Subcommand};
use fyrer::error::FyrerResult;
use fyrer::global;
use fyrer::runner::Runner;

#[derive(Parser)]
#[command(
    name = "fyrer",
    version,
    about = "A declarative, fast and lightweight monorepo tool"
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

fn main() {
    let result = tokio::runtime::Runtime::new()
        .expect("failed to start tokio runtime")
        .block_on(run());
    match result {
        Ok(()) => {
            if global::is_shutting_down() {
                std::process::exit(global::shutdown_code());
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

async fn run() -> FyrerResult<()> {
    let cli = Cli::parse();
    let runner = Runner::load(&cli.config)?;
    match cli.command {
        Command::List => runner.list(),
        Command::Run { task, dry_run } => {
            let tasks = runner.resolve(task.as_deref())?;
            if dry_run {
                return runner.plan(&tasks);
            }
            runner.run(&tasks).await
        }
    }
}
