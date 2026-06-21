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

#[derive(Parser, Debug)]
pub struct RunArgs {
    pub name: String,
}

#[derive(Parser, Debug)]
pub struct DaemonArgs {
    #[arg(long, default_value = "models")]
    pub models_dir: PathBuf,
    #[arg(long, default_value = "scenarios")]
    pub scenarios_dir: PathBuf,
    #[arg(long)]
    pub default_model: Option<String>,
}
