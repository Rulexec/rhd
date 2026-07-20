mod types;
mod handlers;

use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use axum::{routing::post, Router};

pub use types::*;
pub use handlers::chat_completions;

pub async fn start_mock_server() -> (
    u16,
    SharedRequests,
    SharedResponse,
    SharedFlagValue,
    StreamChunkSender,
    SharedAutoStream,
) {
    let requests: SharedRequests = Arc::new(Mutex::new(Vec::new()));
    let response: SharedResponse = Arc::new(Mutex::new(String::new()));
    let flag_value: SharedFlagValue = Arc::new(Mutex::new(true));
    let stream_sender: StreamChunkSender = Arc::new(Mutex::new(None));
    let auto_stream: SharedAutoStream = Arc::new(Mutex::new(true));
    
    let app = Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .with_state((
            requests.clone(),
            response.clone(),
            flag_value.clone(),
            stream_sender.clone(),
            auto_stream.clone(),
        ));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (port, requests, response, flag_value, stream_sender, auto_stream)
}
