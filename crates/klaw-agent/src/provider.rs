use async_trait::async_trait;
use klaw_core::types::{Message, StreamChunk, ToolCall, Usage};
use std::pin::Pin;
use futures::Stream;

/// Request to LLM
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Option<Vec<serde_json::Value>>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stream: bool,
}

/// Non-streaming response from LLM
#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Usage,
    pub model: String,
}

/// Every LLM provider (OpenAI, Anthropic, Ollama, etc.) implements this
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Provider name
    fn name(&self) -> &str;

    /// Non-streaming chat completion
    async fn chat(&self, req: ChatRequest) -> anyhow::Result<ChatResponse>;

    /// Streaming chat completion
    async fn chat_stream(
        &self,
        req: ChatRequest,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<StreamChunk>> + Send>>>;

    /// List available models
    async fn list_models(&self) -> anyhow::Result<Vec<String>>;
}
