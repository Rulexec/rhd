mod cli;
mod client;
mod config;
mod credentials;
mod daemon;
mod execution;
mod ipc;
mod log;
mod mcp_cache;
mod mcp_loader;
mod notifications;
mod project_loader;
mod project_manager;
mod scenario;
mod ws;

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
            let model_aliases = args.parsed_model_aliases();
            let socket_path = args.socket.unwrap_or_else(cli::default_socket_path);
            if let Err(err) = client::run_scenario(&args.name, &socket_path, model_aliases).await {
                if err.to_string() == "ABORTED" {
                    std::process::exit(2);
                }
                eprintln!("error: {err}");
                std::process::exit(1);
            }
        }
        Command::Dev(args) => {
            if let Err(err) = args.validate() {
                eprintln!("error: {err}");
                std::process::exit(1);
            }
            match args.command {
                cli::DevCommand::DaemonNotification => {
                    let socket_path = cli::default_socket_path();
                    if let Err(err) = client::send_daemon_notification(&socket_path).await {
                        eprintln!("error: {err}");
                        std::process::exit(1);
                    }
                    println!("Daemon notification sent");
                }
                cli::DevCommand::FrontendNotification => {
                    let socket_path = cli::default_socket_path();
                    if let Err(err) = client::send_frontend_notification(&socket_path).await {
                        eprintln!("error: {err}");
                        std::process::exit(1);
                    }
                    println!("Frontend notification request sent");
                }
            }
        }
        Command::Reload(args) => {
            let socket_path = args.socket.unwrap_or_else(cli::default_socket_path);
            if let Err(err) = client::send_reload(&socket_path).await {
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

    let credentials = if let Some(cred_path) = &merged.credentials_config {
        if !cred_path.exists() {
            return Err(format!("credentials file not found: {}", cred_path.display()).into());
        }
        credentials::load_credentials(cred_path)?
    } else {
        std::collections::HashMap::new()
    };

    let models = rhd_ai::config::load_models(&merged.models_dir, &credentials)?;
    let scenarios = scenario::load_scenarios_dir(&merged.scenarios_dir)?;
    let mcp_configs = mcp_loader::load_mcp_dir(&merged.mcp_dir)?;
    let projects = project_loader::load_projects(&merged.projects_dir)
        .map_err(|e| format!("failed to load projects: {}", e))?;
    let mcp_cache = std::sync::Arc::new(mcp_cache::McpServerCache::new());
    let project_manager = std::sync::Arc::new(project_manager::ProjectManager::new(projects, mcp_configs.clone(), mcp_cache.clone()));
    let socket_path = args.socket.unwrap_or_else(cli::default_socket_path);
    let db_file = merged.db_dir.join("meta.db");
    let config_paths = daemon::ResolvedConfigPaths {
        config_file: args.config.clone(),
        models_dir: merged.models_dir.clone(),
        scenarios_dir: merged.scenarios_dir.clone(),
        mcp_dir: merged.mcp_dir.clone(),
        projects_dir: merged.projects_dir.clone(),
        credentials_config: merged.credentials_config.clone(),
    };
    daemon::run_daemon(scenarios, models, mcp_configs, merged.default_model, merged.logs, &socket_path, merged.ws_port, db_file.to_str().unwrap_or("db/meta.db"), merged.never_fail, project_manager, config_paths).await?;
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
        mcp_dir: args.mcp_dir.clone().unwrap_or(config.mcp_dir),
        default_model: args.default_model.clone().or(config.default_model),
        logs: args.logs.clone().or(config.logs),
        ws_port: args.ws_port.or(config.ws_port),
        db_dir: args.db_dir.clone().unwrap_or(config.db_dir),
        projects_dir: args.projects_dir.clone().unwrap_or(config.projects_dir),
        credentials_config: config.credentials_config,
        never_fail: config.never_fail,
    }
}
