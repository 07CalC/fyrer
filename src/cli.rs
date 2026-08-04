use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "fyrer",
    version,
    about = "A declarative, fast and lightweight monorepo tool"
)]
pub struct Cli {
    #[arg(short, long, default_value = "fyrer.yml")]
    pub config: String,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Run {
        /// The task to run: `project:task`, a bare `task` name, or empty for all.
        task: Option<String>,
        #[arg(long)]
        dry_run: bool,
    },
    List,
}
