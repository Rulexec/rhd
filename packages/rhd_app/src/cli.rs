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
    #[arg(long, default_value = "rhd.sock")]
    pub socket: PathBuf,
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
    #[arg(long, default_value = "models")]
    pub models_dir: PathBuf,
    #[arg(long, default_value = "scenarios")]
    pub scenarios_dir: PathBuf,
    #[arg(long)]
    pub default_model: Option<String>,
    #[arg(long, default_value_t = false)]
    pub verbose: bool,
    #[arg(long, default_value = "rhd.sock")]
    pub socket: PathBuf,
}

impl DaemonArgs {
    pub fn validate(&self) -> Result<(), String> {
        if !self.models_dir.exists() {
            return Err(format!(
                "models directory does not exist: {}",
                self.models_dir.display()
            ));
        }
        if !self.models_dir.is_dir() {
            return Err(format!(
                "models path is not a directory: {}",
                self.models_dir.display()
            ));
        }
        if !self.scenarios_dir.exists() {
            return Err(format!(
                "scenarios directory does not exist: {}",
                self.scenarios_dir.display()
            ));
        }
        if !self.scenarios_dir.is_dir() {
            return Err(format!(
                "scenarios path is not a directory: {}",
                self.scenarios_dir.display()
            ));
        }
        Ok(())
    }
}
