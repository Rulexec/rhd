use futures_util::StreamExt;
use tokio_util::sync::CancellationToken;

use super::super::types::*;
use super::super::AiError;
use super::StreamExecutor;

impl StreamExecutor {
    pub(crate) async fn chat_stream<F>(
        &self,
        model: &str,
        messages: &[ChatMessage],
        mut on_chunk: F,
        mut raw_log: Option<&mut dyn RawLogger>,
    ) -> Result<StreamResult, AiError>
    where
        F: FnMut(StreamChunk) -> bool,
    {
        let url = format!("{}/chat/completions", self.base_url);

        let request = ChatRequest {
            model,
            messages: messages.to_vec(),
            tools: None,
            stream: true,
        };

        if let Some(ref mut logger) = raw_log {
            if let Ok(json) = serde_json::to_string_pretty(&request) {
                logger.log_request(&json);
            }
        }

        let response = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|source| AiError::Network {
                model: model.to_string(),
                source,
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            if let Some(ref mut logger) = raw_log {
                logger.log_error(Some(status.as_u16()), &body);
            }
            return Err(AiError::Api {
                model: model.to_string(),
                status: status.as_u16(),
                body,
            });
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut finish_reason = None;
        let mut usage = None;
        let mut event_index = 0usize;

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|source| AiError::Network {
                model: model.to_string(),
                source,
            })?;

            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            // Process complete SSE events (separated by \n\n)
            while let Some(event_end) = buffer.find("\n\n") {
                let event = buffer[..event_end].to_string();
                buffer = buffer[event_end + 2..].to_string();

                for line in event.lines() {
                    if line.starts_with("data: ") {
                        let data = &line[6..];

                        if data == "[DONE]" {
                            return Ok(StreamResult {
                                finish_reason,
                                usage,
                            });
                        }

                        if let Some(ref mut logger) = raw_log {
                            logger.log_stream_chunk(event_index, data);
                        }

                        let stream_response: StreamResponse =
                            serde_json::from_str(data).map_err(|source| AiError::JsonParse {
                                model: model.to_string(),
                                source,
                            })?;

                        if let Some(choice) = stream_response.choices.into_iter().next() {
                            let content = choice.delta.content;
                            let reasoning_content = choice.delta.reasoning_content;
                            finish_reason = choice.finish_reason.or(finish_reason);

                            let stream_chunk = StreamChunk {
                                content,
                                reasoning_content,
                                finish_reason: None,
                            };

                            if !on_chunk(stream_chunk) {
                                return Err(AiError::Aborted {
                                    model: model.to_string(),
                                });
                            }
                        }

                        if let Some(u) = stream_response.usage {
                            usage = Some(rhd_api::TokenUsage {
                                prompt_tokens: u.prompt_tokens,
                                completion_tokens: u.completion_tokens,
                                total_tokens: u.total_tokens,
                            });
                        }
                    }
                }
                event_index += 1;
            }
        }

        Ok(StreamResult {
            finish_reason,
            usage,
        })
    }

    pub(crate) async fn chat_stream_cancellable(
        &self,
        model: &str,
        messages: &[ChatMessage],
        cancel: CancellationToken,
        mut on_chunk: impl FnMut(StreamChunk) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>,
        mut raw_log: Option<&mut dyn RawLogger>,
    ) -> Result<StreamResult, AiError> {
        let url = format!("{}/chat/completions", self.base_url);

        let request = ChatRequest {
            model,
            messages: messages.to_vec(),
            tools: None,
            stream: true,
        };

        if let Some(ref mut logger) = raw_log {
            if let Ok(json) = serde_json::to_string_pretty(&request) {
                logger.log_request(&json);
            }
        }

        let response = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|source| AiError::Network {
                model: model.to_string(),
                source,
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            if let Some(ref mut logger) = raw_log {
                logger.log_error(Some(status.as_u16()), &body);
            }
            return Err(AiError::Api {
                model: model.to_string(),
                status: status.as_u16(),
                body,
            });
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut finish_reason = None;
        let mut usage = None;
        let mut event_index = 0usize;

        while let Some(chunk_result) = stream.next().await {
            if cancel.is_cancelled() {
                return Err(AiError::Aborted {
                    model: model.to_string(),
                });
            }

            let chunk = chunk_result.map_err(|source| AiError::StreamError {
                model: model.to_string(),
                event_index,
                message: format!("Failed to read SSE chunk: {}", source),
                source,
            })?;

            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            // Process complete SSE events (separated by \n\n)
            while let Some(event_end) = buffer.find("\n\n") {
                let event = buffer[..event_end].to_string();
                buffer = buffer[event_end + 2..].to_string();

                for line in event.lines() {
                    if line.starts_with("data: ") {
                        let data = &line[6..];

                        if data == "[DONE]" {
                            return Ok(StreamResult {
                                finish_reason,
                                usage,
                            });
                        }

                        if let Some(ref mut logger) = raw_log {
                            logger.log_stream_chunk(event_index, data);
                        }

                        let stream_response: StreamResponse =
                            serde_json::from_str(data).map_err(|source| AiError::JsonParse {
                                model: model.to_string(),
                                source,
                            })?;

                        if let Some(choice) = stream_response.choices.into_iter().next() {
                            let content = choice.delta.content;
                            let reasoning_content = choice.delta.reasoning_content;
                            finish_reason = choice.finish_reason.or(finish_reason);

                            let stream_chunk = StreamChunk {
                                content,
                                reasoning_content,
                                finish_reason: None,
                            };

                            on_chunk(stream_chunk).await;
                        }

                        if let Some(u) = stream_response.usage {
                            usage = Some(rhd_api::TokenUsage {
                                prompt_tokens: u.prompt_tokens,
                                completion_tokens: u.completion_tokens,
                                total_tokens: u.total_tokens,
                            });
                        }
                    }
                }
                event_index += 1;
            }
        }

        Ok(StreamResult {
            finish_reason,
            usage,
        })
    }
}
