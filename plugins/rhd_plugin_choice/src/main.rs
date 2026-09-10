//! Choice plugin for RHD chat system.
//!
//! This plugin provides the `rhd_choice` tool to all chats. It registers the
//! tool per chat; answering the tool calls is the frontend's responsibility.

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use rhd_plugin_choice::plugin;

#[derive(Parser, Debug)]
#[command(name = "rhd_plugin_choice")]
#[command(about = "Choice tool plugin for RHD chat system")]
struct Args {
    /// WebSocket URL of the chat server
    #[arg(long)]
    server_url: String,

    /// Plugin ID (defaults to "choice")
    #[arg(long, default_value = "choice")]
    plugin_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive("rhd_plugin_choice=info".parse()?))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    tracing::info!("Starting rhd_plugin_choice");
    tracing::info!("Server URL: {}", args.server_url);
    tracing::info!("Plugin ID: {}", args.plugin_id);

    plugin::run_plugin(&args.server_url, &args.plugin_id).await?;

    Ok(())
}
