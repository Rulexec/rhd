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
    /// Run frontend UI tests
    Frontend {
        /// WebSocket port for daemon (default: random)
        #[arg(long)]
        ws_port: Option<u16>,

        /// Control server port (default: random)
        #[arg(long)]
        control_port: Option<u16>,
    },
    /// Run simple SSE streaming test
    SseTest,
}
