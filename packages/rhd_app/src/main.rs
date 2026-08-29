//! RHD CLI - Command-line interface for rhd_chat_server

mod cli;
mod commands;

use clap::Parser;
use cli::Cli;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    match cli.command {
        cli::Command::Chats { subcommand } => {
            commands::chats::execute(subcommand).await?;
        }
        cli::Command::Messages { chat_id, all } => {
            commands::messages::execute(chat_id, all).await?;
        }
        cli::Command::Queue { subcommand } => {
            commands::queue::execute(subcommand).await?;
        }
        cli::Command::CreateChat { title, tags } => {
            commands::create_chat::execute(title, tags).await?;
        }
        cli::Command::Plugins { subcommand } => {
            commands::plugins::execute(subcommand).await?;
        }
        cli::Command::Start { config_path } => {
            commands::start::execute(config_path).await?;
        }
    }
    
    Ok(())
}
