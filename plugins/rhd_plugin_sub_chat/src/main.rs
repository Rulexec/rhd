//! Sub-chat plugin for RHD chat system.
//!
//! This plugin provides the `rhd_sub_chat`, `rhd_sub_chat_status` and
//! `rhd_sub_chat_await` tools to all chats. It registers the tools per chat;
//! answering the tool calls arrives with the following phases.

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use rhd_plugin_sub_chat::plugin;

#[derive(Parser, Debug)]
#[command(name = "rhd_plugin_sub_chat")]
#[command(about = "Sub-chat delegation plugin for RHD chat system")]
struct Args {
    /// WebSocket URL of the chat server
    #[arg(long)]
    server_url: String,

    /// Plugin ID (defaults to "rhd_plugin_sub_chat")
    #[arg(long, default_value = "rhd_plugin_sub_chat")]
    plugin_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive("rhd_plugin_sub_chat=info".parse()?))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    tracing::info!("Starting rhd_plugin_sub_chat");
    tracing::info!("Server URL: {}", args.server_url);
    tracing::info!("Plugin ID: {}", args.plugin_id);

    plugin::run_plugin(&args.server_url, &args.plugin_id).await?;

    Ok(())
}
