mod events;
mod handlers;

#[cfg(test)]
mod tests;

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use futures_util::{SinkExt, StreamExt};
use rhd_api::WsEvent;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::time::{interval, Duration};
use tokio_tungstenite::tungstenite::Message;
use tracing::{info, error, instrument};

use crate::daemon::DaemonState;

const PING_INTERVAL_SECS: u64 = 2;
const PONG_TIMEOUT_SECS: u64 = 5;

#[instrument(level = "info", skip(state), fields(addr = %addr))]
pub async fn run_ws_server(
    addr: std::net::SocketAddr,
    state: Arc<DaemonState>,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    info!("WebSocket listening");

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_ws_connection(stream, state).await {
                error!(%peer_addr, %err, "WebSocket connection error");
            }
        });
    }
}

#[instrument(level = "debug", skip(state))]
async fn handle_ws_connection(
    stream: tokio::net::TcpStream,
    state: Arc<DaemonState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    let (mut write, mut read) = ws_stream.split();

    let mut events_rx = state.execution_tracker.subscribe();
    let mut chat_events_rx = state.chat_event_sender.subscribe();
    let mut pause_rx = state.execution_tracker.subscribe_pause();
    let mut ping_interval = interval(Duration::from_secs(PING_INTERVAL_SECS));
    let mut last_pong = Instant::now();

    state.frontend_alive.store(true, Ordering::Relaxed);

    loop {
        tokio::select! {
            _ = ping_interval.tick() => {
                if last_pong.elapsed() > Duration::from_secs(PONG_TIMEOUT_SECS) {
                    state.frontend_alive.store(false, Ordering::Relaxed);
                }
                write.send(Message::Ping(vec![])).await?;
            }
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let response = handlers::handle_ws_message(&text, &state).await;
                        let response_text = serde_json::to_string(&response)?;
                        write.send(Message::Text(response_text)).await?;
                    }
                    Some(Ok(Message::Pong(_))) => {
                        last_pong = Instant::now();
                        state.frontend_alive.store(true, Ordering::Relaxed);
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(e)) => return Err(Box::new(e)),
                    _ => {}
                }
            }
            event = events_rx.recv() => {
                match event {
                    Ok(exec_event) => {
                        let event_name = serde_json::to_string(&exec_event.event)?
                            .trim_matches('"')
                            .to_string();
                        let ws_event = WsEvent::new(
                            &event_name,
                            serde_json::to_value(&exec_event.data)?,
                        );
                        let event_text = serde_json::to_string(&ws_event)?;
                        write.send(Message::Text(event_text)).await?;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            chat_event = chat_events_rx.recv() => {
                match chat_event {
                    Ok(chat_evt) => {
                        if let Some(ws_event) = events::chat_event_to_ws_event(chat_evt, &state) {
                            let event_text = serde_json::to_string(&ws_event)?;
                            write.send(Message::Text(event_text)).await?;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            pause_notification = pause_rx.recv() => {
                match pause_notification {
                    Ok(notification) => {
                        let model_names: Vec<String> = state.inner.read().await.models.iter()
                            .filter(|(_, config)| !config.is_alias)
                            .map(|(name, _)| name.clone())
                            .collect();
                        
                        let scenario_name = state.execution_tracker
                            .get_active_executions()
                            .iter()
                            .find(|e| e.id == notification.execution_id)
                            .map(|e| e.scenario_name.clone())
                            .unwrap_or_default();
                        
                        let payload = serde_json::json!({
                            "executionId": notification.execution_id,
                            "scenarioName": scenario_name,
                            "error": notification.error,
                            "stepName": notification.step_name,
                            "availableModels": model_names,
                        });
                        let ws_event = WsEvent::new("scenarioPaused", payload);
                        let event_text = serde_json::to_string(&ws_event)?;
                        write.send(Message::Text(event_text)).await?;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    state.frontend_alive.store(false, Ordering::Relaxed);
    Ok(())
}
