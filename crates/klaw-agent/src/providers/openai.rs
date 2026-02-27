use crate::provider::{ChatRequest, ChatResponse, LlmProvider};
use async_trait::async_trait;
use futures::Stream;
use klaw_core::types::{StreamChunk, ToolCall, Usage};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use tracing::{debug, info, warn};

/// OpenAI-compatible provider (works with OpenAI, Ollama, Anthropic proxy, etc.)
pub struct OpenAiProvider {
    client: Client,
    base_url: String,
    api_key: String,
    name: String,
}

impl OpenAiProvider {
    pub fn new(base_url: &str, api_key: &str, name: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            name: name.to_string(),
        }
    }

    /// Build the request body
    fn build_body(&self, req: &ChatRequest) -> serde_json::Value {
        let messages: Vec<serde_json::Value> = req.messages.iter().map(|m| {
            let mut msg = serde_json::json!({
                "role": m.role,
                "content": m.content,
            });
            if let Some(ref tool_calls) = m.tool_calls {
                msg["tool_calls"] = serde_json::json!(tool_calls.iter().map(|tc| {
                    serde_json::json!({
                        "id": tc.id,
                        "type": "function",
                        "function": {
                            "name": tc.name,
                            "arguments": tc.arguments.to_string(),
                        }
                    })
                }).collect::<Vec<_>>());
            }
            if let Some(ref tool_call_id) = m.tool_call_id {
                msg["tool_call_id"] = serde_json::json!(tool_call_id);
            }
            msg
        }).collect();

        let mut body = serde_json::json!({
            "model": req.model,
            "messages": messages,
            "stream": req.stream,
        });

        if let Some(temp) = req.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if let Some(max) = req.max_tokens {
            body["max_tokens"] = serde_json::json!(max);
        }
        if let Some(ref tools) = req.tools {
            if !tools.is_empty() {
                body["tools"] = serde_json::json!(tools);
            }
        }

        body
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    fn name(&self) -> &str {
        &self.name
    }

    async fn chat(&self, req: ChatRequest) -> anyhow::Result<ChatResponse> {
        let body = self.build_body(&ChatRequest { stream: false, ..req.clone() });

        let response = self.client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("LLM API error {}: {}", status, error_text);
        }

        let data: serde_json::Value = response.json().await?;
        let choice = &data["choices"][0];

        let content = choice["message"]["content"].as_str().map(|s| s.to_string());

        let tool_calls = if let Some(tcs) = choice["message"]["tool_calls"].as_array() {
            tcs.iter().map(|tc| {
                ToolCall {
                    id: tc["id"].as_str().unwrap_or("").to_string(),
                    name: tc["function"]["name"].as_str().unwrap_or("").to_string(),
                    arguments: serde_json::from_str(
                        tc["function"]["arguments"].as_str().unwrap_or("{}")
                    ).unwrap_or(serde_json::json!({})),
                }
            }).collect()
        } else {
            vec![]
        };

        let usage = if let Some(u) = data.get("usage") {
            Usage {
                input_tokens: u["prompt_tokens"].as_u64().unwrap_or(0),
                output_tokens: u["completion_tokens"].as_u64().unwrap_or(0),
                cache_read_tokens: u["prompt_tokens_details"]["cached_tokens"].as_u64(),
                cache_write_tokens: None,
                total_tokens: u["total_tokens"].as_u64().unwrap_or(0),
            }
        } else {
            Usage::default()
        };

        Ok(ChatResponse {
            content,
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
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("LLM API error {}: {}", status, error_text);
        }

        let stream = async_stream::stream! {
            let mut bytes_stream = response.bytes_stream();
            let mut buffer = String::new();

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

                // Process complete SSE lines
                while let Some(line_end) = buffer.find('\n') {
                    let line = buffer[..line_end].trim().to_string();
                    buffer = buffer[line_end + 1..].to_string();

                    if line.is_empty() || line.starts_with(':') {
                        continue;
                    }

                    if let Some(data) = line.strip_prefix("data: ") {
                        if data.trim() == "[DONE]" {
                            yield Ok(StreamChunk::Done { usage: None });
                            break;
                        }

                        match serde_json::from_str::<serde_json::Value>(data) {
                            Ok(json) => {
                                let delta = &json["choices"][0]["delta"];

                                // Text content
                                if let Some(content) = delta["content"].as_str() {
                                    if !content.is_empty() {
                                        yield Ok(StreamChunk::Text(content.to_string()));
                                    }
                                }

                                // Tool calls
                                if let Some(tool_calls) = delta["tool_calls"].as_array() {
                                    for tc in tool_calls {
                                        let id = tc["id"].as_str().unwrap_or("").to_string();
                                        let name = tc["function"]["name"].as_str().unwrap_or("").to_string();
                                        let args = tc["function"]["arguments"].as_str().unwrap_or("").to_string();

                                        if !name.is_empty() {
                                            yield Ok(StreamChunk::ToolCallStart {
                                                id: id.clone(),
                                                name,
                                            });
                                        }
                                        if !args.is_empty() {
                                            yield Ok(StreamChunk::ToolCallDelta {
                                                id: id.clone(),
                                                arguments: args,
                                            });
                                        }
                                    }
                                }

                                // Usage in final chunk
                                if let Some(u) = json.get("usage") {
                                    let usage = Usage {
                                        input_tokens: u["prompt_tokens"].as_u64().unwrap_or(0),
                                        output_tokens: u["completion_tokens"].as_u64().unwrap_or(0),
                                        cache_read_tokens: None,
                                        cache_write_tokens: None,
                                        total_tokens: u["total_tokens"].as_u64().unwrap_or(0),
                                    };
                                    yield Ok(StreamChunk::Done { usage: Some(usage) });
                                }
                            }
                            Err(e) => {
                                debug!("Skipping unparseable SSE data: {}", e);
                            }
                        }
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }

    async fn list_models(&self) -> anyhow::Result<Vec<String>> {
        let response = self.client
            .get(format!("{}/models", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;

        if !response.status().is_success() {
            return Ok(vec![]);
        }

        let data: serde_json::Value = response.json().await?;
        let models = data["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        Ok(models)
    }
}
