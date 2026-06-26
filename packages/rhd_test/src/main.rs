mod args;
mod mcp_test;
mod mock_server;
mod standard_test;
mod utils;

use clap::Parser;

use args::Args;
use mcp_test::run_mcp_test;
use mock_server::start_mock_server;
use standard_test::run_single_test;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let (port, requests, response, flag_value) = start_mock_server().await;
    println!("Mock AI server started on port {port}");

    let mut failures: Vec<u64> = Vec::new();
    let mut mcp_failures: Vec<u64> = Vec::new();

    for i in 0..args.repetitions {
        let iter_seed = args.seed + i as u64;

        requests.lock().unwrap().clear();

        let (failed, log) =
            run_single_test(iter_seed, port, requests.clone(), response.clone()).await;

        if failed {
            println!(
                "\n=== Repetition {}/{} (seed: {}) FAILED ===",
                i + 1,
                args.repetitions,
                iter_seed
            );
            print!("{log}");
            failures.push(iter_seed);
        }

        requests.lock().unwrap().clear();

        let (mcp_failed, mcp_log) =
            run_mcp_test(iter_seed, port, requests.clone(), response.clone(), flag_value.clone()).await;

        if mcp_failed {
            println!(
                "\n=== MCP Test Repetition {}/{} (seed: {}) FAILED ===",
                i + 1,
                args.repetitions,
                iter_seed
            );
            print!("{mcp_log}");
            mcp_failures.push(iter_seed);
        }
    }

    println!("\n=== Summary ===");
    println!(
        "Standard Test - Total: {}, Passed: {}, Failed: {}",
        args.repetitions,
        args.repetitions as usize - failures.len(),
        failures.len()
    );
    println!(
        "MCP Test - Total: {}, Passed: {}, Failed: {}",
        args.repetitions,
        args.repetitions as usize - mcp_failures.len(),
        mcp_failures.len()
    );

    if !failures.is_empty() || !mcp_failures.is_empty() {
        if !failures.is_empty() {
            eprintln!("Standard test failed seeds: {:?}", failures);
            eprintln!(
                "\nTo reproduce first standard test failure:"
            );
            eprintln!(
                "  cargo run -p rhd_test -- --seed {} --repetitions 1",
                failures[0]
            );
        }
        if !mcp_failures.is_empty() {
            eprintln!("MCP test failed seeds: {:?}", mcp_failures);
            eprintln!(
                "\nTo reproduce first MCP test failure:"
            );
            eprintln!(
                "  cargo run -p rhd_test -- --seed {} --repetitions 1",
                mcp_failures[0]
            );
        }
        eprintln!("\nE2E TEST FAILED");
        std::process::exit(1);
    } else {
        println!("\nE2E TEST PASSED");
    }
}
