use axum::{
    response::sse::{Event, Sse},
    routing::post,
    Router,
};
use std::convert::Infallible;
use tokio::net::TcpListener;
use tokio_stream::wrappers::ReceiverStream;

async fn sse_handler() -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    // Control channel (like mock_server)
    let (control_tx, mut control_rx) = tokio::sync::mpsc::channel::<Option<String>>(10);
    
    // Event channel (like mock_server)
    let (event_tx, event_rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(100);
    
    // Spawn task to convert control chunks to SSE events (like mock_server)
    tokio::spawn(async move {
        loop {
            match control_rx.recv().await {
                Some(chunk) => {
                    match chunk {
                        Some(content) => {
                            let json = format!(r#"{{"choices":[{{"delta":{{"content":"{}"}}}}]}}"#, content);
                            if event_tx.send(Ok(Event::default().data(json))).await.is_err() {
                                break;
                            }
                        }
                        None => {
                            let _ = event_tx.send(Ok(Event::default().data("[DONE]"))).await;
                            break;
                        }
                    }
                }
                None => break,
            }
        }
    });
    
    // Spawn task to send chunks with delay (simulating control server)
    // Using 2 second initial delay to simulate e2e timing (daemon connects, frontend loads,
    // test polls /stream-ready, then sends chunk)
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        let _ = control_tx.send(Some("Hello".to_string())).await;
        
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        let _ = control_tx.send(Some(" World".to_string())).await;
        
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        let _ = control_tx.send(None).await;
    });
    
    let stream = ReceiverStream::new(event_rx);
    // NOTE: No keep-alive here (like mock_server)
    Sse::new(stream)
}

pub async fn run_sse_test() {
    println!("[sse_test] Starting simple SSE test");
    
    // Start server
    let app = Router::new().route("/chat/completions", post(sse_handler));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    println!("[sse_test] Server started on port {}", port);
    
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    
    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Connect with rhd_ai client
    let client = rhd_ai::client::OpenAiClient::new(
        format!("http://127.0.0.1:{}", port),
        "test-key",
    );
    
    let messages = vec![rhd_ai::client::ChatMessage::user("test")];
    
    println!("[sse_test] Connecting with client");
    
    let mut chunks = Vec::new();
    let result = client
        .chat_stream("test-model", &messages, |chunk| {
            println!("[sse_test] Received chunk: {:?}", chunk);
            if let Some(content) = chunk.content {
                chunks.push(content);
            }
            true
        })
        .await;
    
    match result {
        Ok(stream_result) => {
            println!("[sse_test] Stream completed successfully");
            println!("[sse_test] Finish reason: {:?}", stream_result.finish_reason);
            println!("[sse_test] Collected chunks: {:?}", chunks);
            
            let full_content = chunks.join("");
            if full_content == "Hello World" {
                println!("[sse_test] ✓ Test PASSED - received correct content");
            } else {
                println!("[sse_test] ✗ Test FAILED - expected 'Hello World', got '{}'", full_content);
            }
        }
        Err(e) => {
            println!("[sse_test] ✗ Test FAILED with error: {}", e);
        }
    }
}
