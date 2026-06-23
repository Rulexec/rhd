mod cli;
mod client;
mod config;
mod daemon;
mod ipc;
mod log;
mod scenario;

use std::path::PathBuf;

use clap::Parser;
use cli::{Cli, Command};

use crate::config::DaemonConfig;

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
    let config = load_config(&args.config)?;
    let merged = merge_config(config, &args);

    let models = rhd_ai::config::load_models(&merged.models_dir)?;
    let scenarios = scenario::load_scenarios_dir(&merged.scenarios_dir)?;
    let socket_path = args.socket.unwrap_or_else(cli::default_socket_path);
    daemon::run_daemon(scenarios, models, merged.default_model, merged.logs, &socket_path).await?;
    Ok(())
}

fn load_config(path: &PathBuf) -> Result<DaemonConfig, String> {
    if path.exists() {
        config::load_config(path)
    } else if path.as_os_str() == "rhd.yaml" {
        Ok(DaemonConfig::default())
    } else {
        Err(format!("config file not found: {}", path.display()))
    }
}

fn merge_config(config: DaemonConfig, args: &cli::DaemonArgs) -> DaemonConfig {
    DaemonConfig {
        models_dir: args.models_dir.clone().unwrap_or(config.models_dir),
        scenarios_dir: args.scenarios_dir.clone().unwrap_or(config.scenarios_dir),
        default_model: args.default_model.clone().or(config.default_model),
        logs: args.logs.clone().or(config.logs),
    }
}
