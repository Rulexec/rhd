use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod config;
mod plugin;

#[derive(Parser, Debug)]
#[command(name = "rhd_plugin_ai_completions")]
#[command(about = "AI completions plugin for RHD chat system")]
struct Args {
    /// WebSocket URL of the chat server
    #[arg(long)]
    server_url: String,

    /// Path to configuration file
    #[arg(long)]
    config: String,

    /// Plugin ID (defaults to "ai_completions")
    #[arg(long, default_value = "ai_completions")]
    plugin_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive("rhd_plugin_ai_completions=info".parse()?))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Parse arguments
    let args = Args::parse();

    // Load configuration
    let config = config::load_config(&args.config)?;

    tracing::info!("Starting AI completions plugin");
    tracing::info!("Server URL: {}", args.server_url);
    tracing::info!("Plugin ID: {}", args.plugin_id);

    // Run plugin
    plugin::run_plugin(&args.server_url, &args.plugin_id, config).await?;

    Ok(())
}
