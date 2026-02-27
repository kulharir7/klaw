use crate::provider::ChatRequest;
use serde::{Deserialize, Serialize};

/// Thinking configuration for extended reasoning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    #[serde(rename = "type")]
    pub thinking_type: String,
    pub budget_tokens: u64,
}

/// Thinking level presets
#[derive(Debug, Clone, Default)]
pub enum ThinkingLevel {
    #[default]
    Off,
    Low,
    Medium,
    High,
}

impl ThinkingLevel {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "low" | "min" => Self::Low,
            "medium" | "med" | "default" => Self::Medium,
            "high" | "max" => Self::High,
            _ => Self::Off,
        }
    }

    /// Get thinking budget tokens for this level
    pub fn budget_tokens(&self) -> Option<u64> {
        match self {
            Self::Off => None,
            Self::Low => Some(1024),
            Self::Medium => Some(4096),
            Self::High => Some(16384),
        }
    }

    /// Apply thinking to a ChatRequest
    pub fn apply_to_request(&self, req: &mut ChatRequest) {
        req.thinking = self.budget_tokens().map(|budget| ThinkingConfig {
            thinking_type: "enabled".to_string(),
            budget_tokens: budget,
        });
    }
}
