use crate::provider::{ChatRequest, ChatResponse, LlmProvider};
use async_trait::async_trait;
use futures::Stream;
use klaw_core::types::{Role, StreamChunk, ToolCall, Usage};
use reqwest::Client;
use std::pin::Pin;
use tracing::{debug, info, warn};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Native Anthropic/Claude provider
pub struct AnthropicProvider {
    client: Client,
    api_key: String,
}

impl AnthropicProvider {
    pub fn new(api_key: &str) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.to_string(),
        }
    }

    fn build_body(&self, req: &ChatRequest) -> serde_json::Value {
        // Anthropic uses separate system param, not in messages array
        let mut system_text = String::new();
        let messages: Vec<serde_json::Value> = req.messages.iter().filter_map(|m| {
            match m.role {
                Role::System => {
                    system_text = m.content.clone();
                    None
                }
                Role::User => Some(serde_json::json!({
                    "role": "user",
                    "content": m.content,
                })),
                Role::Assistant => {
                    let mut msg = serde_json::json!({
                        "role": "assistant",
                    });
                    // If has tool_use content
                    if let Some(ref tcs) = m.tool_calls {
                        let mut content: Vec<serde_json::Value> = Vec::new();
                        if !m.content.is_empty() {
                            content.push(serde_json::json!({"type": "text", "text": m.content}));
                        }
                        for tc in tcs {
                            content.push(serde_json::json!({
                                "type": "tool_use",
                                "id": tc.id,
                                "name": tc.name,
                                "input": tc.arguments,
                            }));
                        }
                        msg["content"] = serde_json::json!(content);
                    } else {
                        msg["content"] = serde_json::json!(m.content);
                    }
                    Some(msg)
                }
                Role::Tool => {
                    Some(serde_json::json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": m.tool_call_id,
                            "content": m.content,
                        }]
                    }))
                }
            }
        }).collect();

        let mut body = serde_json::json!({
            "model": req.model,
            "messages": messages,
            "max_tokens": req.max_tokens.unwrap_or(4096),
            "stream": req.stream,
        });

        if !system_text.is_empty() {
            body["system"] = serde_json::json!(system_text);
        }
        if let Some(temp) = req.temperature {
            body["temperature"] = serde_json::json!(temp);
        }

        // Convert OpenAI tool format → Anthropic tool format
        if let Some(ref tools) = req.tools {
            let anthropic_tools: Vec<serde_json::Value> = tools.iter().filter_map(|t| {
                let func = t.get("function")?;
                Some(serde_json::json!({
                    "name": func["name"],
                    "description": func["description"],
                    "input_schema": func["parameters"],
                }))
            }).collect();
            if !anthropic_tools.is_empty() {
                body["tools"] = serde_json::json!(anthropic_tools);
            }
        }

        body
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn chat(&self, req: ChatRequest) -> anyhow::Result<ChatResponse> {
        let body = self.build_body(&ChatRequest { stream: false, ..req.clone() });

        let response = self.client
            .post(format!("{}/messages", ANTHROPIC_API_URL))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error {}: {}", status, error_text);
        }

        let data: serde_json::Value = response.json().await?;

        let mut content_text = String::new();
        let mut tool_calls = Vec::new();

        if let Some(content) = data["content"].as_array() {
            for block in content {
                match block["type"].as_str() {
                    Some("text") => {
                        if let Some(text) = block["text"].as_str() {
                            content_text.push_str(text);
                        }
                    }
                    Some("tool_use") => {
                        tool_calls.push(ToolCall {
                            id: block["id"].as_str().unwrap_or("").to_string(),
                            name: block["name"].as_str().unwrap_or("").to_string(),
                            arguments: block["input"].clone(),
                        });
                    }
                    _ => {}
                }
            }
        }

        let usage = Usage {
            input_tokens: data["usage"]["input_tokens"].as_u64().unwrap_or(0),
            output_tokens: data["usage"]["output_tokens"].as_u64().unwrap_or(0),
            cache_read_tokens: data["usage"]["cache_read_input_tokens"].as_u64(),
            cache_write_tokens: data["usage"]["cache_creation_input_tokens"].as_u64(),
            total_tokens: data["usage"]["input_tokens"].as_u64().unwrap_or(0)
                + data["usage"]["output_tokens"].as_u64().unwrap_or(0),
        };

        Ok(ChatResponse {
            content: if content_text.is_empty() { None } else { Some(content_text) },
            tool_calls,
            usage,
            model: data["model"].as_str().unwrap_or(&req.model).to_string(),
        })
    }

    async fn chat_stream(
        &self,
        req: ChatRequest,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<StreamChunk>> + Send>>> {
        let body = self.build_body(&ChatRequest { stream: true, ..req.clone() });

        let response = self.client
            .post(format!("{}/messages", ANTHROPIC_API_URL))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error {}: {}", status, error_text);
        }

        let stream = async_stream::stream! {
            let mut bytes_stream = response.bytes_stream();
            let mut buffer = String::new();
            let mut current_tool_id = String::new();
            let mut current_tool_name = String::new();

            use futures::StreamExt;
            while let Some(chunk) = bytes_stream.next().await {
                let chunk = match chunk {
                    Ok(c) => c,
                    Err(e) => {
                        yield Err(anyhow::anyhow!("Stream error: {}", e));
                        break;
                    }
                };

                buffer.push_str(&String::from_utf8_lossy(&chunk));

                while let Some(line_end) = buffer.find('\n') {
                    let line = buffer[..line_end].trim().to_string();
                    buffer = buffer[line_end + 1..].to_string();

                    if line.is_empty() || line.starts_with("event:") {
                        continue;
                    }

                    if let Some(data) = line.strip_prefix("data: ") {
                        match serde_json::from_str::<serde_json::Value>(data) {
                            Ok(json) => {
                                let event_type = json["type"].as_str().unwrap_or("");

                                match event_type {
                                    "content_block_start" => {
                                        let block = &json["content_block"];
                                        if block["type"].as_str() == Some("tool_use") {
                                            current_tool_id = block["id"].as_str().unwrap_or("").to_string();
                                            current_tool_name = block["name"].as_str().unwrap_or("").to_string();
                                            yield Ok(StreamChunk::ToolCallStart {
                                                id: current_tool_id.clone(),
                                                name: current_tool_name.clone(),
                                            });
                                        }
                                    }
                                    "content_block_delta" => {
                                        let delta = &json["delta"];
                                        match delta["type"].as_str() {
                                            Some("text_delta") => {
                                                if let Some(text) = delta["text"].as_str() {
                                                    yield Ok(StreamChunk::Text(text.to_string()));
                                                }
                                            }
                                            Some("input_json_delta") => {
                                                if let Some(partial) = delta["partial_json"].as_str() {
                                                    yield Ok(StreamChunk::ToolCallDelta {
                                                        id: current_tool_id.clone(),
                                                        arguments: partial.to_string(),
                                                    });
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                    "content_block_stop" => {
                                        if !current_tool_id.is_empty() {
                                            yield Ok(StreamChunk::ToolCallEnd {
                                                id: current_tool_id.clone(),
                                            });
                                            current_tool_id.clear();
                                            current_tool_name.clear();
                                        }
                                    }
                                    "message_delta" => {
                                        // Final usage
                                        if let Some(u) = json.get("usage") {
                                            let usage = Usage {
                                                input_tokens: 0, // input tokens come in message_start
                                                output_tokens: u["output_tokens"].as_u64().unwrap_or(0),
                                                cache_read_tokens: None,
                                                cache_write_tokens: None,
                                                total_tokens: u["output_tokens"].as_u64().unwrap_or(0),
                                            };
                                            yield Ok(StreamChunk::Done { usage: Some(usage) });
                                        }
                                    }
                                    "message_stop" => {
                                        yield Ok(StreamChunk::Done { usage: None });
                                    }
                                    _ => {}
                                }
                            }
                            Err(_) => {}
                        }
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }

    async fn list_models(&self) -> anyhow::Result<Vec<String>> {
        // Anthropic doesn't have a models endpoint, return known models
        Ok(vec![
            "claude-sonnet-4-20250514".to_string(),
            "claude-opus-4-20250514".to_string(),
            "claude-haiku-3-5-20241022".to_string(),
        ])
    }
}
