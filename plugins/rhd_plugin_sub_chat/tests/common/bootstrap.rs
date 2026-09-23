//! Bootstrap plumbing for the full-pipeline e2e environment: plugin ids, the
//! in-memory chat server, the temp credentials/config files, tracing init,
//! and the sub-chat ack probe. Consumed by [`super::full_env`].

use std::io::Write as _;
use std::time::Duration;

use rhd_chat_api::SendCustomEventParams;
use rhd_chat_client::{ChatClient, PluginsMonitor};
use tokio::time::timeout;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Plugin ids (plan note 3: two plugins with distinct ids, helper as a third
/// registered connection that acks all custom events).
pub const AI_PLUGIN_ID: &str = "test_ai";
pub const SUB_CHAT_PLUGIN_ID: &str = "test_sub_chat";
pub const HELPER_PLUGIN_ID: &str = "test_helper";

/// Poll bound for every convergence check — generous per plan note 2 (fixed
/// sleeps for convergence are banned).
pub const WAIT_TIMEOUT: Duration = Duration::from_secs(20);

/// Marker of the bootstrap sentinel chat; never scripted with responses, so
/// sentinel traffic parks harmlessly instead of touching scenario assertions.
pub const SENTINEL_MARKER: &str = "e2e:bootstrap-sentinel";

/// Tracing init (same idiom as the ai_completions e2e tests; level from
/// `RUST_LOG`, silent by default).
pub fn init_tracing() {
    let _ = tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .try_init();
}

/// Emit `e2e:bootstrap-probe` and wait until the sub-chat plugin has
/// acknowledged it. The helper acks via its own subscription; ai_completions
/// never subscribes to custom events by production design (it only sends),
/// so both are excluded from the expected set — what remains, exactly
/// `test_sub_chat`, must ack.
pub async fn ack_probe(client: &ChatClient, monitor: &PluginsMonitor) {
    let event = client
        .send_custom_event(SendCustomEventParams {
            event_name: "e2e:bootstrap-probe".to_string(),
            additional: Some(r#"{"probe":true}"#.to_string()),
            chat_id: None,
            message_id: None,
            tool_call_id: None,
        })
        .await
        .expect("probe sendCustomEvent failed");
    timeout(
        Duration::from_secs(15),
        monitor.wait_for_acks_except(
            &event.event_id,
            &[HELPER_PLUGIN_ID, AI_PLUGIN_ID],
            Duration::from_secs(14),
        ),
    )
    .await
    .expect("probe ack-wait itself timed out")
    .expect("the sub-chat plugin never acknowledged the live probe event");
}

/// An ephemeral-port, in-memory chat server; returns its ws:// URL. The
/// server task detaches for the process lifetime (same handling as the
/// ai_completions e2e tests, whose `start` handle is dropped).
pub async fn start_chat_server() -> String {
    let chat_config = rhd_chat_server::config::Config {
        host: "127.0.0.1".to_string(),
        port: 0,
        db_path: ":memory:".to_string(),
        clear_pending_acks: false,
    };
    let (port, _server_handle) = rhd_chat_server::server::start(chat_config)
        .await
        .expect("chat server must start");
    format!("ws://127.0.0.1:{port}/")
}

/// Temp credentials + plugin YAML pointing the default model at the mock
/// provider (verbatim shape from `tool_call_e2e_test.rs`). The files must
/// outlive the plugin tasks, hence the returned handles are stored.
pub fn write_plugin_config(
    base_url: &str,
) -> (
    rhd_plugin_ai_completions::config::PluginConfig,
    tempfile::NamedTempFile,
    tempfile::NamedTempFile,
) {
    let mut creds_file = tempfile::NamedTempFile::new().expect("creds temp file");
    writeln!(creds_file, "testApiKey: test-api-key-12345").expect("write creds");

    let mut config_file = tempfile::NamedTempFile::new().expect("config temp file");
    write!(
        config_file,
        r#"
credentialsConfig: {}
ai_completions:
  models:
    default:
      alias: test
    test:
      baseUrl: "{}"
      apiKey:
        cred: testApiKey
      model: "test-model"
"#,
        creds_file.path().to_str().unwrap(),
        base_url
    )
    .expect("write config");

    let config =
        rhd_plugin_ai_completions::config::load_config(config_file.path().to_str().unwrap())
            .expect("plugin config must load");
    (config, config_file, creds_file)
}
