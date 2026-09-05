//! MCP servers plugin for RHD chat system.
//!
//! Spawns configured MCP servers, registers their tools on eligible chats
//! (prefixed `<name>:`), and executes tool calls, pushing results back.

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod config;
mod plugin;

#[derive(Parser, Debug)]
#[command(name = "rhd_plugin_mcp")]
#[command(about = "MCP servers plugin for RHD chat system")]
struct Args {
    /// WebSocket URL of the chat server
    #[arg(long)]
    server_url: String,

    /// Plugin ID (defaults to "mcp")
    #[arg(long, default_value = "mcp")]
    plugin_id: String,

    /// Only serve chats tagged `worktree:<workTreeId>`. When omitted,
    /// serve only chats that carry no `worktree:*` tag.
    #[arg(long)]
    worktree: Option<String>,

    /// Path to configuration file
    #[arg(long)]
    config: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive("rhd_plugin_mcp=info".parse()?))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    tracing::info!("Starting rhd_plugin_mcp");
    tracing::info!("Server URL: {}", args.server_url);
    tracing::info!("Plugin ID: {}", args.plugin_id);
    tracing::info!("Worktree filter: {:?}", args.worktree);

    let config = config::load_config(&args.config)?;
    tracing::info!("Loaded {} MCP server(s)", config.servers.len());

    plugin::run_plugin(
        &args.server_url,
        &args.plugin_id,
        args.worktree.as_deref(),
        config,
    )
    .await?;

    Ok(())
}
