//! Todo list plugin for RHD chat system.
//!
//! This plugin manages a task tracking list for multi-step operations.
//! It triggers on newly created chats to inject a system message and register
//! the `rhd_set_todo_list` tool, then handles tool calls to update the todo list.

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod parser;
mod plugin;
mod templates;
mod todo_store;
mod tool_handler;

#[derive(Parser, Debug)]
#[command(name = "rhd_plugin_todo_list")]
#[command(about = "Todo list plugin for RHD chat system")]
struct Args {
    /// WebSocket URL of the chat server
    #[arg(long)]
    server_url: String,

    /// Plugin ID (defaults to "todo_list")
    #[arg(long, default_value = "todo_list")]
    plugin_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive("rhd_plugin_todo_list=info".parse()?))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Parse arguments
    let args = Args::parse();

    tracing::info!("Starting rhd_plugin_todo_list");
    tracing::info!("Server URL: {}", args.server_url);
    tracing::info!("Plugin ID: {}", args.plugin_id);

    // Run the plugin
    plugin::run_plugin(&args.server_url, &args.plugin_id).await?;

    Ok(())
}
