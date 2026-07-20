mod setup;
mod execution;
mod validation;

use std::sync::{Arc, Mutex};

use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::mock_server::{SharedRequests, SharedResponse};
use crate::utils::{create_temp_script, generate_random_string};

pub async fn run_single_test(
    iter_seed: u64,
    port: u16,
    requests: SharedRequests,
    response: SharedResponse,
) -> (bool, String) {
    let mut log = String::new();
    let mut rng = StdRng::seed_from_u64(iter_seed);

    let random_response = generate_random_string(&mut rng, 8);
    *response.lock().unwrap() = random_response.clone();
    log.push_str(&format!("  AI response: {random_response}\n"));

    let temp_dir = tempfile::tempdir().unwrap();
    create_temp_script(temp_dir.path(), &mut rng);
    log.push_str(&format!(
        "  Temp scripts at: {}\n",
        temp_dir.path().display()
    ));

    let setup_result = setup::setup_daemon(port, temp_dir.path(), &mut log).await;
    
    match setup_result {
        Ok(setup) => {
            let (execution_result, daemon_stdout_content) = execution::run_scenario(
                &setup,
                &random_response,
                &mut log,
            ).await;
            
            validation::validate_results(
                &setup,
                &execution_result,
                &daemon_stdout_content,
                &random_response,
                requests,
                &mut log,
            );
            
            setup.cleanup().await;
            (execution_result.failed, log)
        }
        Err(err) => {
            log.push_str("  FAIL: Daemon startup failed\n");
            log.push_str(&format!("  {err}\n"));
            (true, log)
        }
    }
}
