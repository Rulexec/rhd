use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rhd_test", about = "E2E test runner for rhd")]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Random seed for deterministic test generation
    #[arg(long, default_value_t = 42)]
    pub seed: u64,

    /// Number of test repetitions
    #[arg(long, default_value_t = 10)]
    pub repetitions: u32,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run simple SSE streaming test
    SseTest,
    /// Run mock MCP server (stdio JSON-RPC)
    McpServer,
    /// Run daemon startup test only
    DaemonStartup,
}
