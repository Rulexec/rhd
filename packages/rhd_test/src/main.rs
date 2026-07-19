mod args;
mod control_server;
mod daemon_startup_test;
mod frontend_test;
mod mcp_test;
mod mock_mcp_server;
mod mock_server;
mod sse_test;
mod standard_test;
mod utils;

use clap::Parser;

use args::{Args, Commands};
use frontend_test::run_frontend_test;
use mcp_test::run_mcp_test;
use mock_server::start_mock_server;
use standard_test::run_single_test;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    match args.command {
        Some(Commands::Frontend { ws_port, control_port }) => {
            let success = run_frontend_test(ws_port, control_port).await;
            if !success {
                eprintln!("\nFRONTEND TEST FAILED");
                std::process::exit(1);
            } else {
                println!("\nFRONTEND TEST PASSED");
            }
        }
        Some(Commands::SseTest) => {
            sse_test::run_sse_test().await;
        }
        Some(Commands::McpServer) => {
            mock_mcp_server::run_mock_mcp_server();
        }
        Some(Commands::DaemonStartup) => {
            let (failed, log) = daemon_startup_test::run_daemon_startup_test().await;
            print!("{log}");
            if failed {
                eprintln!("\nDAEMON STARTUP TEST FAILED");
                std::process::exit(1);
            } else {
                println!("\nDAEMON STARTUP TEST PASSED");
            }
        }
        None => {
            run_standard_tests(args.seed, args.repetitions).await;
        }
    }
}

async fn run_standard_tests(seed: u64, repetitions: u32) {
    let (port, requests, response, flag_value, _stream_sender, _auto_stream) = start_mock_server().await;
    println!("Mock AI server started on port {port}");

    let mut failures: Vec<u64> = Vec::new();
    let mut mcp_failures: Vec<u64> = Vec::new();

    for i in 0..repetitions {
        let iter_seed = seed + i as u64;

        requests.lock().unwrap().clear();

        let (failed, log) =
            run_single_test(iter_seed, port, requests.clone(), response.clone()).await;

        if failed {
            println!(
                "\n=== Repetition {}/{} (seed: {}) FAILED ===",
                i + 1,
                repetitions,
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
                repetitions,
                iter_seed
            );
            print!("{mcp_log}");
            mcp_failures.push(iter_seed);
        }
    }

    println!("\n=== Summary ===");
    println!(
        "Standard Test - Total: {}, Passed: {}, Failed: {}",
        repetitions,
        repetitions as usize - failures.len(),
        failures.len()
    );
    println!(
        "MCP Test - Total: {}, Passed: {}, Failed: {}",
        repetitions,
        repetitions as usize - mcp_failures.len(),
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
