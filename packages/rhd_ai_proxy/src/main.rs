use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use rhd_ai_proxy::config::Config;
use rhd_ai_proxy::proxy::{build_router, ProxyState};

#[derive(Parser)]
#[command(name = "rhd_ai_proxy")]
#[command(about = "OpenAI-compatible proxy that injects per-model extraBody and pipes streaming responses")]
struct Args {
    /// Path to the proxy YAML config
    #[arg(long)]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let config = Config::load(&args.config)?;

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let state = Arc::new(ProxyState::new(&config.proxy)?);
    let app = build_router(state);

    let addr = format!("127.0.0.1:{}", config.proxy.port);
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!(
        %addr,
        target = %config.proxy.target.path,
        models = config.proxy.models.len(),
        "rhd_ai_proxy listening"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutdown signal received");
}
