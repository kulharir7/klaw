use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Inbound message from any channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    pub id: String,
    pub channel: String,
    pub chat_id: String,
    pub sender_id: String,
    pub sender_name: Option<String>,
    pub text: Option<String>,
    pub media: Option<Vec<Media>>,
    pub reply_to: Option<String>,
    pub chat_type: ChatType,
    pub timestamp: DateTime<Utc>,
}

/// Outbound message to any channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    pub channel: String,
    pub chat_id: String,
    pub text: String,
    pub reply_to: Option<String>,
    pub media: Option<Vec<Media>>,
    pub buttons: Option<Vec<Button>>,
    pub silent: bool,
}

/// Media attachment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Media {
    pub url: Option<String>,
    pub path: Option<String>,
    pub data: Option<Vec<u8>>,
    pub mime_type: String,
    pub filename: Option<String>,
    pub caption: Option<String>,
}

/// Inline button
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Button {
    pub text: String,
    pub callback_data: Option<String>,
    pub url: Option<String>,
}

/// Chat type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChatType {
    Direct,
    Group,
    Channel,
}

/// Session key
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct SessionKey(pub String);

impl SessionKey {
    pub fn main(agent_id: &str) -> Self {
        Self(format!("agent:{}:main", agent_id))
    }

    pub fn group(agent_id: &str, channel: &str, chat_id: &str) -> Self {
        Self(format!("agent:{}:{}:{}", agent_id, channel, chat_id))
    }
}

/// Message role in conversation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// A single message in conversation history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub role: Role,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl Message {
    pub fn system(content: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role: Role::System,
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: None,
            timestamp: Utc::now(),
        }
    }

    pub fn user(content: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role: Role::User,
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: None,
            timestamp: Utc::now(),
        }
    }

    pub fn assistant(content: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role: Role::Assistant,
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: None,
            timestamp: Utc::now(),
        }
    }
}

/// Tool call from LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub content: String,
    pub is_error: bool,
}

/// LLM streaming chunk
#[derive(Debug, Clone)]
pub enum StreamChunk {
    Text(String),
    ToolCallStart { id: String, name: String },
    ToolCallDelta { id: String, arguments: String },
    ToolCallEnd { id: String },
    Done { usage: Option<Usage> },
    Error(String),
}

/// Token usage
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub total_tokens: u64,
}
