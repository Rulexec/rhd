//! Entry point for the system prompt plugin.

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use rhd_plugin_system_prompt::{config, plugin};

#[derive(Parser, Debug)]
#[command(name = "rhd_plugin_system_prompt")]
#[command(about = "System prompt plugin for RHD chat system")]
struct Args {
    /// WebSocket URL of the chat server
    #[arg(long)]
    server_url: String,

    /// Path to configuration file
    #[arg(long)]
    config: String,

    /// Plugin ID (defaults to "system_prompt")
    #[arg(long, default_value = "system_prompt")]
    plugin_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive("rhd_plugin_system_prompt=info".parse()?))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Parse arguments
    let args = Args::parse();

    // Load configuration
    let (config, cached_prompts) = config::load_config(&args.config)?;

    tracing::info!("Starting system prompt plugin");
    tracing::info!("Server URL: {}", args.server_url);
    tracing::info!("Plugin ID: {}", args.plugin_id);
    tracing::info!("Loaded {} system prompts", cached_prompts.len());

    // Run plugin
    plugin::run_plugin(&args.server_url, &args.plugin_id, config, cached_prompts).await?;

    Ok(())
}
