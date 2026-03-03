use klaw_core::types::{Message, Role};
use chrono::Utc;
use tracing::debug;

/// Configuration for context pruning
#[derive(Debug, Clone)]
pub struct PruningConfig {
    pub mode: String,
    pub ttl_seconds: u64,
    pub keep_last_assistants: usize,
    pub max_tool_result_chars: usize,
    /// Importance threshold (0-100)
    pub importance_threshold: u32,
    /// Keep system messages
    pub keep_system: bool,
    /// Keep user messages
    pub keep_user: bool,
    /// Max total tokens
    pub max_tokens: usize,
}

impl Default for PruningConfig {
    fn default() -> Self {
        Self {
            mode: "cache-ttl".to_string(),
            ttl_seconds: 600,
            keep_last_assistants: 4,
            max_tool_result_chars: 8000,
            importance_threshold: 50,
            keep_system: true,
            keep_user: true,
            max_tokens: 128_000,
        }
    }
}

impl PruningConfig {
    /// Create aggressive pruning config
    pub fn aggressive() -> Self {
        Self {
            mode: "aggressive".to_string(),
            ttl_seconds: 300,
            keep_last_assistants: 2,
            max_tool_result_chars: 2000,
            importance_threshold: 70,
            keep_system: true,
            keep_user: false,
            max_tokens: 32_000,
        }
    }
    
    /// Create conservative pruning config
    pub fn conservative() -> Self {
        Self {
            mode: "conservative".to_string(),
            ttl_seconds: 1800,
            keep_last_assistants: 8,
            max_tool_result_chars: 16_000,
            importance_threshold: 30,
            keep_system: true,
            keep_user: true,
            max_tokens: 200_000,
        }
    }
    
    /// No pruning
    pub fn none() -> Self {
        Self {
            mode: "off".to_string(),
            ttl_seconds: u64::MAX,
            keep_last_assistants: usize::MAX,
            max_tool_result_chars: usize::MAX,
            importance_threshold: 0,
            keep_system: true,
            keep_user: true,
            max_tokens: usize::MAX,
        }
    }
}

/// Prune old tool results from context to save tokens
pub fn prune_context(messages: &mut Vec<Message>, config: &PruningConfig) {
    if config.mode == "off" {
        return;
    }

    let now = Utc::now();
    let len = messages.len();

    // Find the index where "protected" zone starts (last N assistant messages)
    let mut assistant_count = 0;
    let mut protect_from = len;
    for i in (0..len).rev() {
        if matches!(messages[i].role, Role::Assistant) {
            assistant_count += 1;
            if assistant_count >= config.keep_last_assistants {
                protect_from = i;
                break;
            }
        }
    }

    for i in 0..protect_from {
        if !matches!(messages[i].role, Role::Tool) {
            continue;
        }

        let age_secs = (now - messages[i].timestamp).num_seconds().max(0) as u64;

        if age_secs > config.ttl_seconds * 3 {
            // Very old — clear entirely
            if messages[i].content != "[Old tool result cleared]" {
                debug!("Clearing very old tool result at index {}", i);
                messages[i].content = "[Old tool result cleared]".to_string();
            }
        } else if messages[i].content.len() > config.max_tool_result_chars {
            // Trim long results
            let content = &messages[i].content;
            let trimmed = format!(
                "{}...{}",
                &content[..1500.min(content.len())],
                &content[content.len().saturating_sub(1500)..]
            );
            debug!("Trimming tool result at index {} from {} to {} chars", i, content.len(), trimmed.len());
            messages[i].content = trimmed;
        }
    }
}
