//! The `fyrer` command-line entry point.

use clap::{Parser, Subcommand};
use fyrer::{error::FyrerResult, global, runner::Runner};

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
        /// The task to run: `project:task`, a bare `task` name, or empty for all.
        task: Option<String>,
        #[arg(long)]
        dry_run: bool,
    },
    List,
}

fn main() {
    let exit_code = match tokio::runtime::Runtime::new() {
        Ok(runtime) => match runtime.block_on(real_main()) {
            Ok(()) if global::is_shutting_down() => global::shutdown_code(),
            Ok(()) => 0,
            Err(error) => {
                eprintln!("error: {error}");
                1
            }
        },
        Err(error) => {
            eprintln!("failed to start tokio runtime: {error}");
            1
        }
    };
    std::process::exit(exit_code);
}

async fn real_main() -> FyrerResult<()> {
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
