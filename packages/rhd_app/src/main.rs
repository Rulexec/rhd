mod cli;
mod client;
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
            if let Err(err) = args.validate() {
                eprintln!("error: {err}");
                std::process::exit(1);
            }
            if let Err(err) = run_daemon_command(args).await {
                eprintln!("error: {err}");
                std::process::exit(1);
            }
        }
        Command::Run(args) => {
            if let Err(err) = args.validate() {
                eprintln!("error: {err}");
                std::process::exit(1);
            }
            let socket_path = args.socket.unwrap_or_else(cli::default_socket_path);
            if let Err(err) = client::run_scenario(&args.name, &socket_path).await {
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
    let socket_path = args.socket.unwrap_or_else(cli::default_socket_path);
    daemon::run_daemon(scenarios, models, args.default_model, args.verbose, &socket_path).await?;
    Ok(())
}
