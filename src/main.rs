use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use color_eyre::eyre::Context;
use fyrer::{Task, app::App, cli::Cli, error::FyrerResult, global, logger::LogMessage};
// fn main() { let exit_code = match tokio::runtime::Runtime::new() {
//         Ok(runtime) => match runtime.block_on(real_main()) {
//             Ok(()) if global::is_shutting_down() => global::shutdown_code(),
//             Ok(()) => 0,
//             Err(error) => {
//                 eprintln!("error: {error}");
//                 1
//             }
//         },
//         Err(error) => {
//             eprintln!("failed to start tokio runtime: {error}");
//             1
//         }
//     };
//     std::process::exit(exit_code);
// }

// async fn real_main() -> FyrerResult<()> {
//     let cli = Cli::parse();
//     let runner = Runner::load(&cli.config)?;
//     match cli.command {
//         Command::List => runner.list(),
//         Command::Run { task, dry_run } => {
//             let tasks = runner.resolve(task.as_deref())?;
//             if dry_run {
//                 return runner.plan(&tasks);
//             }
//             runner.run(&tasks).await
//         }
//     }
// }

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut app = App::init(&cli.config)?;
    app.start(cli.command)?;
    Ok(())
}
