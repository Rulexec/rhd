use futures_util::StreamExt;
use rhd_ai_client::{AiClient, ChatCompletionRequest, ChatMessage, StreamChunk};
use rhd_mock_ai_provider::{MockAiProvider, MockAiResponse, SimpleListener, StreamBuilder};
use std::time::{Duration, Instant};
use tokio::time::timeout;

/// Test that streaming responses are delivered incrementally, not buffered.
///
/// This test creates a mock server that sends chunks with delays between them.
/// If the client properly streams, we should receive chunks as they arrive.
/// If the client buffers, we'll receive all chunks at once after the full delay.
#[tokio::test]
async fn test_streaming_delivers_chunks_incrementally() {
    // Create a stream builder that will send chunks with delays
    let stream_builder = StreamBuilder::new()
        .text("Hello")
        .text_delayed(", ", Duration::from_millis(100))
        .text_delayed("world!", Duration::from_millis(100))
        .finish("stop");

    let listener = SimpleListener::new();
    listener.push_response(MockAiResponse::Stream(stream_builder.build()));
    let provider = MockAiProvider::start(listener).await.unwrap();
    let client = AiClient::new(provider.base_url(), "test-key");

    let request = ChatCompletionRequest {
        model: "test-model".to_string(),
        messages: vec![ChatMessage::User {
            content: "Hi".to_string(),
        }],
        tools: None,
        stream: true,
    };

    let start = Instant::now();
    let mut stream = client.chat_completion_stream(request).await.unwrap();

    // Collect chunks with timing information
    let mut chunks_with_timing = Vec::new();
    
    // First chunk should arrive relatively quickly (within 200ms)
    let first_chunk = timeout(Duration::from_millis(200), stream.next())
        .await
        .expect("First chunk should arrive within 200ms")
        .expect("Stream should yield a chunk")
        .expect("Chunk should be Ok");
    
    let first_elapsed = start.elapsed();
    chunks_with_timing.push((first_chunk, first_elapsed));

    // Second chunk should arrive after some delay (but not all at once)
    let second_chunk = timeout(Duration::from_millis(300), stream.next())
        .await
        .expect("Second chunk should arrive within 300ms of first")
        .expect("Stream should yield a chunk")
        .expect("Chunk should be Ok");
    
    let second_elapsed = start.elapsed();
    chunks_with_timing.push((second_chunk, second_elapsed));

    // Third chunk (final)
    let third_chunk = timeout(Duration::from_millis(300), stream.next())
        .await
        .expect("Third chunk should arrive within 300ms of second")
        .expect("Stream should yield a chunk")
        .expect("Chunk should be Ok");
    
    let third_elapsed = start.elapsed();
    chunks_with_timing.push((third_chunk, third_elapsed));

    // Verify we got all chunks
    assert_eq!(chunks_with_timing.len(), 3);
    
    // Verify content
    assert_eq!(chunks_with_timing[0].0.content.as_deref(), Some("Hello"));
    assert_eq!(chunks_with_timing[1].0.content.as_deref(), Some(", "));
    assert_eq!(chunks_with_timing[2].0.content.as_deref(), Some("world!"));
    assert_eq!(chunks_with_timing[2].0.finish_reason.as_deref(), Some("stop"));

    // CRITICAL: Verify chunks arrived incrementally, not all at once
    // If buffered, all chunks would arrive at ~300ms (total delay)
    // If streaming, first chunk arrives at ~0ms, second at ~100ms, third at ~200ms
    
    // First chunk should arrive much earlier than the total time
    assert!(
        chunks_with_timing[0].1 < Duration::from_millis(150),
        "First chunk arrived too late: {:?}. Expected < 150ms if streaming properly",
        chunks_with_timing[0].1
    );

    // Total time should be at least 200ms (sum of delays)
    let total_time = chunks_with_timing[2].1;
    assert!(
        total_time >= Duration::from_millis(180),
        "Total time too short: {:?}. Expected >= 180ms",
        total_time
    );

    provider.shutdown().await;
}

/// Test that streaming handles the [DONE] marker correctly
#[tokio::test]
async fn test_streaming_handles_done_marker() {
    let stream_builder = StreamBuilder::new()
        .text("Test")
        .finish("stop");

    let listener = SimpleListener::new();
    listener.push_response(MockAiResponse::Stream(stream_builder.build()));
    let provider = MockAiProvider::start(listener).await.unwrap();
    let client = AiClient::new(provider.base_url(), "test-key");

    let request = ChatCompletionRequest {
        model: "test-model".to_string(),
        messages: vec![ChatMessage::User {
            content: "Hi".to_string(),
        }],
        tools: None,
        stream: true,
    };

    let mut stream = client.chat_completion_stream(request).await.unwrap();

    // Should get one chunk
    let chunk = stream.next().await.unwrap().unwrap();
    assert_eq!(chunk.content.as_deref(), Some("Test"));
    assert_eq!(chunk.finish_reason.as_deref(), Some("stop"));

    // Stream should end (no more chunks)
    let next = timeout(Duration::from_millis(100), stream.next()).await;
    assert!(next.is_err() || next.unwrap().is_none(), "Stream should end after [DONE]");

    provider.shutdown().await;
}

/// Test that streaming handles reasoning content
#[tokio::test]
async fn test_streaming_with_reasoning_content() {
    let stream_builder = StreamBuilder::new()
        .chunk(StreamChunk {
            reasoning_content: Some("Thinking...".to_string()),
            ..Default::default()
        })
        .chunk(StreamChunk {
            content: Some("Answer".to_string()),
            finish_reason: Some("stop".to_string()),
            ..Default::default()
        });

    let listener = SimpleListener::new();
    listener.push_response(MockAiResponse::Stream(stream_builder.build()));
    let provider = MockAiProvider::start(listener).await.unwrap();
    let client = AiClient::new(provider.base_url(), "test-key");

    let request = ChatCompletionRequest {
        model: "test-model".to_string(),
        messages: vec![ChatMessage::User {
            content: "Hi".to_string(),
        }],
        tools: None,
        stream: true,
    };

    let mut stream = client.chat_completion_stream(request).await.unwrap();

    // First chunk should have reasoning content
    let chunk1 = stream.next().await.unwrap().unwrap();
    assert_eq!(chunk1.reasoning_content.as_deref(), Some("Thinking..."));
    assert!(chunk1.content.is_none());

    // Second chunk should have content
    let chunk2 = stream.next().await.unwrap().unwrap();
    assert_eq!(chunk2.content.as_deref(), Some("Answer"));
    assert!(chunk2.reasoning_content.is_none());

    provider.shutdown().await;
}
