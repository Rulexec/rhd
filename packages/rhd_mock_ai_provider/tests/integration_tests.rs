use std::time::Duration;

use rhd_ai_client::{AiClient, ChatCompletionRequest, ChatMessage, ToolDefinition};
use rhd_mock_ai_provider::{MockAiProvider, RecordingListener, SimpleListener, StreamBuilder};

#[tokio::test]
async fn test_server_starts_on_random_port() {
    let listener = SimpleListener::new();
    let provider = MockAiProvider::start(listener).await.unwrap();

    assert!(provider.port() > 0);
    assert!(provider.base_url().starts_with("http://127.0.0.1:"));

    provider.shutdown().await;
}

#[tokio::test]
async fn test_non_streaming_text_completion() {
    let listener = RecordingListener::new();
    listener.push_text("Hello, world!");

    let provider = MockAiProvider::start(listener.clone()).await.unwrap();
    let client = AiClient::new(provider.base_url(), "test-key");

    let request = ChatCompletionRequest {
        model: "test-model".to_string(),
        messages: vec![ChatMessage::User {
            content: "Hi".to_string(),
        }],
        tools: None,
        stream: false,
    };

    let response = client.chat_completion(request).await.unwrap();
    assert_eq!(
        response.choices[0].message.content.as_deref(),
        Some("Hello, world!")
    );

    // Verify request was recorded
    let requests = listener.get_requests();
    assert_eq!(requests.len(), 1);

    provider.shutdown().await;
}

#[tokio::test]
async fn test_streaming_with_predefined_chunks() {
    let listener = SimpleListener::new();

    // Use StreamBuilder to create a stream with delays
    let stream = StreamBuilder::new()
        .text("Hello")
        .text_delayed(", ", Duration::from_millis(100))
        .text_delayed("world!", Duration::from_millis(100))
        .finish("stop")
        .build();

    listener.push_stream_receiver(stream);

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

    let mut chunks = Vec::new();
    let start = std::time::Instant::now();
    let result = client
        .chat_completion_stream(request, |chunk| {
            chunks.push(chunk);
            true
        })
        .await
        .unwrap();

    let elapsed = start.elapsed();

    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0].content.as_deref(), Some("Hello"));
    assert_eq!(chunks[1].content.as_deref(), Some(", "));
    assert_eq!(chunks[2].content.as_deref(), Some("world!"));
    assert_eq!(result.finish_reason.as_deref(), Some("stop"));

    // Verify delays were applied (should take at least 200ms)
    assert!(elapsed >= Duration::from_millis(200));

    provider.shutdown().await;
}

#[tokio::test]
async fn test_tool_call_response() {
    let listener = RecordingListener::new();
    listener.push_tool_call("get_weather", r#"{"city":"London"}"#);

    let provider = MockAiProvider::start(listener.clone()).await.unwrap();
    let client = AiClient::new(provider.base_url(), "test-key");

    let request = ChatCompletionRequest {
        model: "test-model".to_string(),
        messages: vec![ChatMessage::User {
            content: "What's the weather?".to_string(),
        }],
        tools: Some(vec![ToolDefinition::function(
            "get_weather",
            "Get weather for a city",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "city": {"type": "string"}
                }
            }),
        )]),
        stream: false,
    };

    let response = client.chat_completion(request).await.unwrap();
    assert_eq!(
        response.choices[0].finish_reason.as_deref(),
        Some("tool_calls")
    );

    let tool_calls = response.choices[0]
        .message
        .tool_calls
        .as_ref()
        .unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].function.name, "get_weather");
    assert_eq!(tool_calls[0].function.arguments, r#"{"city":"London"}"#);

    provider.shutdown().await;
}

#[tokio::test]
async fn test_tool_call_with_result() {
    let listener = RecordingListener::new();

    // First response: tool call
    listener.push_tool_call("get_weather", r#"{"city":"London"}"#);
    // Second response: text after tool result
    listener.push_text("The weather in London is sunny.");

    let provider = MockAiProvider::start(listener.clone()).await.unwrap();
    let client = AiClient::new(provider.base_url(), "test-key");

    // First request - should get tool call
    let request1 = ChatCompletionRequest {
        model: "test-model".to_string(),
        messages: vec![ChatMessage::User {
            content: "What's the weather?".to_string(),
        }],
        tools: Some(vec![ToolDefinition::function(
            "get_weather",
            "Get weather",
            serde_json::json!({}),
        )]),
        stream: false,
    };

    let response1 = client.chat_completion(request1).await.unwrap();
    let tool_call = &response1.choices[0]
        .message
        .tool_calls
        .as_ref()
        .unwrap()[0];

    // Second request - with tool result
    let request2 = ChatCompletionRequest {
        model: "test-model".to_string(),
        messages: vec![
            ChatMessage::User {
                content: "What's the weather?".to_string(),
            },
            ChatMessage::Assistant {
                content: None,
                tool_calls: Some(vec![tool_call.clone()]),
            },
            ChatMessage::Tool {
                tool_call_id: tool_call.id.clone(),
                content: r#"{"temperature":"20C","condition":"sunny"}"#.to_string(),
            },
        ],
        tools: None,
        stream: false,
    };

    let response2 = client.chat_completion(request2).await.unwrap();
    assert_eq!(
        response2.choices[0].message.content.as_deref(),
        Some("The weather in London is sunny.")
    );

    // Verify both requests were recorded
    let requests = listener.get_requests();
    assert_eq!(requests.len(), 2);

    provider.shutdown().await;
}

#[tokio::test]
async fn test_multiple_sequential_requests() {
    let listener = RecordingListener::new();
    listener.push_text("First response");
    listener.push_text("Second response");
    listener.push_text("Third response");

    let provider = MockAiProvider::start(listener.clone()).await.unwrap();
    let client = AiClient::new(provider.base_url(), "test-key");

    for i in 1..=3 {
        let request = ChatCompletionRequest {
            model: "test-model".to_string(),
            messages: vec![ChatMessage::User {
                content: format!("Message {}", i),
            }],
            tools: None,
            stream: false,
        };

        let response = client.chat_completion(request).await.unwrap();
        let expected = match i {
            1 => "First response",
            2 => "Second response",
            3 => "Third response",
            _ => unreachable!(),
        };
        assert_eq!(
            response.choices[0].message.content.as_deref(),
            Some(expected)
        );
    }

    assert_eq!(listener.get_requests().len(), 3);

    provider.shutdown().await;
}
