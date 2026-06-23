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
}

impl RunArgs {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("scenario name must not be empty".to_string());
        }
        Ok(())
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
    pub default_model: Option<String>,
    #[arg(long)]
    pub logs: Option<PathBuf>,
    #[arg(long)]
    pub socket: Option<PathBuf>,
}

impl DaemonArgs {
    pub fn validate(&self) -> Result<(), String> {
        Ok(())
    }
}
