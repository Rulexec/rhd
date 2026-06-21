mod cli;
mod daemon;
mod ipc;
mod scenario;

use clap::Parser;
use cli::{Cli, Command};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Daemon(args) => {
            if let Err(err) = run_daemon_command(args).await {
                eprintln!("error: {err}");
                std::process::exit(1);
            }
        }
    }
}

async fn run_daemon_command(
    args: cli::DaemonArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let models = rhd_ai::config::load_models(&args.models_dir)?;
    let scenarios = scenario::load_scenarios_dir(&args.scenarios_dir)?;
    daemon::run_daemon(scenarios, models, args.default_model).await?;
    Ok(())
}
