//! Ollama Provider - Supports both local Ollama and Ollama Cloud
//! 
//! Ollama API: https://ollama.com/api/chat
//! Local: http://localhost:11434/api/chat

use crate::provider::{ChatRequest, ChatResponse, LlmProvider};
use async_trait::async_trait;
use futures::Stream;
use klaw_core::types::{StreamChunk, ToolCall, Usage};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use tracing::{debug, info, warn, error};

/// Ollama provider (local and cloud)
pub struct OllamaProvider {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    name: String,
}

impl OllamaProvider {
    pub fn new(base_url: &str, api_key: &str, name: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: if api_key.is_empty() { None } else { Some(api_key.to_string()) },
            name: name.to_string(),
        }
    }
    
    /// Convert ChatRequest to Ollama format
    fn to_ollama_request(&self, req: &ChatRequest, model: &str) -> serde_json::Value {
        let messages: Vec<serde_json::Value> = req.messages.iter().map(|m| {
            serde_json::json!({
                "role": m.role,
                "content": m.content,
            })
        }).collect();
        
        let mut body = serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": req.stream,
        });
        
        if let Some(ref tools) = req.tools {
            if !tools.is_empty() {
                // Ollama uses a different tools format
                body["tools"] = serde_json::json!(tools.iter().map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t["function"]["name"],
                            "description": t["function"]["description"],
                            "parameters": t["function"]["parameters"],
                        }
                    })
                }).collect::<Vec<_>>());
            }
        }
        
        if let Some(temp) = req.temperature {
            body["options"] = serde_json::json!({
                "temperature": temp,
            });
        }
        
        body
    }
    
    /// Get API endpoint based on provider
    fn get_endpoint(&self) -> String {
        // For Ollama Cloud, use the base URL + /api/chat
        // For local Ollama, use localhost:11434/api/chat
        if self.base_url.contains("ollama.com") {
            // Ollama Cloud
            format!("{}/api/chat", self.base_url.trim_end_matches('/').trim_end_matches("/v1"))
        } else {
            // Local Ollama
            format!("{}/api/chat", self.base_url.trim_end_matches('/'))
        }
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn chat(&self, req: ChatRequest) -> anyhow::Result<ChatResponse> {
        let body = self.to_ollama_request(&req, &req.model);
        let endpoint = self.get_endpoint();
        
        let mut request = self.client
            .post(&endpoint)
            .header("Content-Type", "application/json");
            
        // Add auth header if API key is present
        if let Some(ref key) = self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }
        
        let response = request
            .json(&body)
            .send()
            .await?;
            
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!("Ollama API error {}: {}", status, error_text);
            anyhow::bail!("Ollama API error {}: {}", status, error_text);
        }
        
        let data: serde_json::Value = response.json().await?;
        debug!("Ollama response: {:?}", data);
        
        // Parse Ollama response
        let message = data.get("message")
            .ok_or_else(|| anyhow::anyhow!("No message in Ollama response"))?;
        
        let content = message.get("content")
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());
            
        let tool_calls: Vec<ToolCall> = if let Some(tc) = message.get("tool_calls") {
            tc.as_array()
                .unwrap_or(&vec![])
                .iter()
                .map(|t| ToolCall {
                    id: t.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string(),
                    name: t.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()).unwrap_or("").to_string(),
                    arguments: t.get("function").and_then(|f| f.get("arguments")).cloned().unwrap_or(serde_json::json!({})),
                })
                .collect()
        } else {
            vec![]
        };
        
        Ok(ChatResponse {
            content,
            tool_calls,
            usage: Usage {
                input_tokens: data.get("prompt_eval_count").and_then(|p| p.as_u64()).unwrap_or(0),
                output_tokens: data.get("eval_count").and_then(|e| e.as_u64()).unwrap_or(0),
                cache_read_tokens: None,
                cache_write_tokens: None,
                total_tokens: 0,
            },
            model: req.model,
        })
    }
    
    async fn chat_stream(&self, req: ChatRequest) -> anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<StreamChunk>> + Send>>> {
        // For now, return non-streaming as Ollama streaming requires different handling
        // Use `chat` method instead or implement proper SSE parsing
        let response = self.chat(req).await?;
        
        // Create a simple stream that yields the full content
        let content = response.content.clone().unwrap_or_default();
        
        let stream = async_stream::stream! {
            if !content.is_empty() {
                yield Ok(StreamChunk::Text(content));
            }
            yield Ok(StreamChunk::Done { usage: Some(response.usage) });
        };
        
        Ok(Box::pin(stream))
    }
    
    async fn list_models(&self) -> anyhow::Result<Vec<String>> {
        let endpoint = if self.base_url.contains("ollama.com") {
            "https://ollama.com/api/tags".to_string()
        } else {
            format!("{}/api/tags", self.base_url)
        };
        
        let mut request = self.client.get(&endpoint);
        
        if let Some(ref key) = self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }
        
        let response = request.send().await?;
        
        if !response.status().is_success() {
            anyhow::bail!("Failed to list models: {}", response.status());
        }
        
        let data: serde_json::Value = response.json().await?;
        
        let models = data.get("models")
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
            
        Ok(models)
    }
}