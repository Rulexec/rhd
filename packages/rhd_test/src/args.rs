use clap::Parser;

#[derive(Parser)]
#[command(name = "rhd_test", about = "E2E test runner for rhd")]
pub struct Args {
    /// Random seed for deterministic test generation
    #[arg(long, default_value_t = 42)]
    pub seed: u64,

    /// Number of test repetitions
    #[arg(long, default_value_t = 10)]
    pub repetitions: u32,
}
