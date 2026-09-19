//! Entry point for the slash-commands plugin.

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use rhd_plugin_commands::{config, plugin};

#[derive(Parser, Debug)]
#[command(name = "rhd_plugin_commands")]
#[command(about = "Slash-command plugin for RHD queued messages")]
struct Args {
    /// WebSocket URL of the chat server
    #[arg(long)]
    server_url: String,

    /// Path to configuration file
    #[arg(long)]
    config: String,

    /// Plugin ID (defaults to "commands")
    #[arg(long, default_value = "commands")]
    plugin_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive("rhd_plugin_commands=info".parse()?))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Parse arguments
    let args = Args::parse();

    // Fail fast on bad config / missing prompt files
    let registry = config::load_config(&args.config)?;

    tracing::info!("Starting commands plugin");
    tracing::info!("Server URL: {}", args.server_url);
    tracing::info!("Plugin ID: {}", args.plugin_id);
    tracing::info!("Loaded {} commands", registry.len());

    plugin::run_plugin(&args.server_url, &args.plugin_id, registry).await?;
    Ok(())
}
