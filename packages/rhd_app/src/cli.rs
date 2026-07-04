use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "rhd", about = "rhd daemon")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Daemon(DaemonArgs),
    Run(RunArgs),
    Dev(DevArgs),
}

pub fn default_socket_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("rhd.sock")
}

#[derive(Parser, Debug)]
pub struct RunArgs {
    pub name: String,
    #[arg(long)]
    pub socket: Option<PathBuf>,
    #[arg(long = "modelAlias", value_name = "ALIAS=TARGET")]
    pub model_aliases: Vec<String>,
}

impl RunArgs {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("scenario name must not be empty".to_string());
        }
        for entry in &self.model_aliases {
            let parts: Vec<&str> = entry.splitn(2, '=').collect();
            if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
                return Err(format!("invalid --modelAlias format: '{entry}', expected ALIAS=TARGET"));
            }
        }
        Ok(())
    }

    pub fn parsed_model_aliases(&self) -> Vec<(String, String)> {
        self.model_aliases
            .iter()
            .filter_map(|entry| {
                let parts: Vec<&str> = entry.splitn(2, '=').collect();
                if parts.len() == 2 {
                    Some((parts[0].to_string(), parts[1].to_string()))
                } else {
                    None
                }
            })
            .collect()
    }
}

#[derive(Parser, Debug)]
pub struct DaemonArgs {
    #[arg(long, default_value = "rhd.yaml")]
    pub config: PathBuf,
    #[arg(long)]
    pub models_dir: Option<PathBuf>,
    #[arg(long)]
    pub scenarios_dir: Option<PathBuf>,
    #[arg(long)]
    pub mcp_dir: Option<PathBuf>,
    #[arg(long)]
    pub default_model: Option<String>,
    #[arg(long)]
    pub logs: Option<PathBuf>,
    #[arg(long)]
    pub socket: Option<PathBuf>,
    #[arg(long)]
    pub ws_port: Option<u16>,
    #[arg(long)]
    pub db_dir: Option<PathBuf>,
}

impl DaemonArgs {
    pub fn validate(&self) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Parser, Debug)]
pub struct DevArgs {
    #[command(subcommand)]
    pub command: DevCommand,
}

#[derive(Subcommand, Debug)]
pub enum DevCommand {
    DaemonNotification,
    FrontendNotification,
}

impl DevArgs {
    pub fn validate(&self) -> Result<(), String> {
        Ok(())
    }
}
