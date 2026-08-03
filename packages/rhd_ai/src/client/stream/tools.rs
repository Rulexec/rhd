use futures_util::StreamExt;
use tokio_util::sync::CancellationToken;

use super::super::types::*;
use super::super::AiError;
use super::StreamExecutor;

impl StreamExecutor {
    pub(crate) async fn chat_stream_with_tools(
        &self,
        model: &str,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        cancel: CancellationToken,
        mut on_chunk: impl FnMut(StreamChunk) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>,
        mut raw_log: Option<&mut dyn RawLogger>,
    ) -> Result<StreamResultWithTools, AiError> {
        let url = format!("{}/chat/completions", self.base_url);

        let request = ChatRequest {
            model,
            messages: messages.to_vec(),
            tools: if tools.is_empty() { None } else { Some(tools) },
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
        let mut accumulated_content = String::new();
        let mut tool_call_accumulators: Vec<ToolCallAccumulator> = Vec::new();

        loop {
            if cancel.is_cancelled() {
                return Err(AiError::Aborted {
                    model: model.to_string(),
                });
            }
            let chunk_result = tokio::select! {
                chunk = stream.next() => match chunk {
                    Some(c) => c,
                    None => break,
                },
                _ = cancel.cancelled() => {
                    return Err(AiError::Aborted {
                        model: model.to_string(),
                    });
                }
            };

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
                            let tool_calls = tool_call_accumulators
                                .into_iter()
                                .enumerate()
                                .filter_map(|(idx, acc)| acc.build(idx))
                                .collect();

                            return Ok(StreamResultWithTools {
                                finish_reason,
                                usage,
                                content: if accumulated_content.is_empty() {
                                    None
                                } else {
                                    Some(accumulated_content)
                                },
                                tool_calls,
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

                            if let Some(ref c) = content {
                                accumulated_content.push_str(c);
                            }

                            let stream_chunk = StreamChunk {
                                content,
                                reasoning_content,
                                finish_reason: None,
                            };

                            on_chunk(stream_chunk).await;

                            // Accumulate tool call deltas
                            if let Some(tool_call_deltas) = choice.delta.tool_calls {
                                for delta in tool_call_deltas {
                                    let index = delta.index;
                                    
                                    // Ensure we have an accumulator for this index
                                    while tool_call_accumulators.len() <= index {
                                        tool_call_accumulators.push(ToolCallAccumulator::default());
                                    }

                                    let acc = &mut tool_call_accumulators[index];
                                    
                                    if let Some(id) = delta.id {
                                        acc.id = Some(id);
                                    }
                                    if let Some(call_type) = delta.call_type {
                                        acc.call_type = Some(call_type);
                                    }
                                    if let Some(function_delta) = delta.function {
                                        if let Some(name) = function_delta.name {
                                            acc.function_name = Some(name);
                                        }
                                        if let Some(arguments) = function_delta.arguments {
                                            acc.arguments.push_str(&arguments);
                                        }
                                    }
                                }
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

        let tool_calls = tool_call_accumulators
            .into_iter()
            .enumerate()
            .filter_map(|(idx, acc)| acc.build(idx))
            .collect();

        Ok(StreamResultWithTools {
            finish_reason,
            usage,
            content: if accumulated_content.is_empty() {
                None
            } else {
                Some(accumulated_content)
            },
            tool_calls,
        })
    }
}

#[derive(Default)]
struct ToolCallAccumulator {
    id: Option<String>,
    call_type: Option<String>,
    function_name: Option<String>,
    arguments: String,
}

impl ToolCallAccumulator {
    fn build(self, index: usize) -> Option<ToolCall> {
        let id = self.id.filter(|s| !s.is_empty()).unwrap_or_else(|| format!("call_{}", index));
        let call_type = self.call_type.unwrap_or_else(|| "function".to_string());
        let function_name = self.function_name?;

        Some(ToolCall {
            id,
            call_type,
            function: FunctionCall {
                name: function_name,
                arguments: self.arguments,
            },
        })
    }
}
