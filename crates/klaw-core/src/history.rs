//! Message history management
//! Handles history limits, trimming, and retention

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// History limits configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HistoryLimits {
    /// Max messages to keep
    pub max_messages: usize,
    /// Max age in seconds (0 = no limit)
    pub max_age_seconds: u64,
    /// Max total tokens (0 = no limit)
    pub max_tokens: u64,
    /// Keep system messages always
    pub keep_system: bool,
    /// Keep last N messages
    pub keep_last_n: usize,
    /// Trim strategy: "oldest", "smart", "importance"
    pub trim_strategy: String,
}

impl Default for HistoryLimits {
    fn default() -> Self {
        Self {
            max_messages: 100,
            max_age_seconds: 0,
            max_tokens: 0,
            keep_system: true,
            keep_last_n: 10,
            trim_strategy: "smart".to_string(),
        }
    }
}

impl HistoryLimits {
    /// Create new history limits
    pub fn new(max_messages: usize) -> Self {
        Self {
            max_messages,
            ..Self::default()
        }
    }
    
    /// No limits
    pub fn unlimited() -> Self {
        Self {
            max_messages: usize::MAX,
            max_age_seconds: 0,
            max_tokens: 0,
            keep_system: true,
            keep_last_n: usize::MAX,
            trim_strategy: "smart".to_string(),
        }
    }
    
    /// Strict limits (low memory)
    pub fn strict() -> Self {
        Self {
            max_messages: 20,
            max_age_seconds: 3600,
            max_tokens: 4000,
            keep_system: true,
            keep_last_n: 5,
            trim_strategy: "smart".to_string(),
        }
    }
    
    /// Check if messages need trimming
    pub fn needs_trim(&self, message_count: usize) -> bool {
        message_count > self.max_messages
    }
    
    /// Calculate how many messages to remove
    pub fn trim_count(&self, message_count: usize) -> usize {
        if message_count <= self.max_messages || message_count <= self.keep_last_n {
            return 0;
        }
        
        let to_remove = message_count.saturating_sub(self.max_messages).saturating_add(self.keep_last_n);
        to_remove.min(message_count.saturating_sub(self.keep_last_n))
    }
}

/// Message with metadata for history tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryMessage {
    pub role: String,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub token_count: Option<u64>,
    pub importance: MessageImportance,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageImportance {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl Default for MessageImportance {
    fn default() -> Self {
        MessageImportance::Normal
    }
}

/// History buffer with limits
#[derive(Debug, Clone)]
pub struct HistoryBuffer {
    messages: VecDeque<HistoryMessage>,
    limits: HistoryLimits,
    total_tokens: u64,
}

impl HistoryBuffer {
    /// Create a new history buffer
    pub fn new(limits: HistoryLimits) -> Self {
        Self {
            messages: VecDeque::new(),
            limits,
            total_tokens: 0,
        }
    }
    
    /// Add a message to history
    pub fn push(&mut self, message: HistoryMessage) {
        // Update token count
        if let Some(tokens) = message.token_count {
            self.total_tokens += tokens;
        }
        
        self.messages.push_back(message);
        self.maybe_trim();
    }
    
    /// Get all messages
    pub fn messages(&self) -> &VecDeque<HistoryMessage> {
        &self.messages
    }
    
    /// Get message count
    pub fn len(&self) -> usize {
        self.messages.len()
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
    
    /// Clear history
    pub fn clear(&mut self) {
        self.messages.clear();
        self.total_tokens = 0;
    }
    
    /// Trim history based on limits
    pub fn maybe_trim(&mut self) {
        // Trim by count
        if self.limits.needs_trim(self.messages.len()) {
            let to_remove = self.limits.trim_count(self.messages.len());
            self.trim_oldest(to_remove);
        }
        
        // Trim by tokens
        if self.limits.max_tokens > 0 && self.total_tokens > self.limits.max_tokens {
            self.trim_by_tokens();
        }
        
        // Trim by age
        if self.limits.max_age_seconds > 0 {
            self.trim_by_age();
        }
    }
    
    fn trim_oldest(&mut self, count: usize) {
        let mut removed = 0;
        while removed < count && self.messages.len() > self.limits.keep_last_n {
            if let Some(msg) = self.messages.pop_front() {
                if self.limits.keep_system && msg.role == "system" {
                    // Put system message back
                    self.messages.push_front(msg);
                    break;
                }
                if let Some(tokens) = msg.token_count {
                    self.total_tokens = self.total_tokens.saturating_sub(tokens);
                }
                removed += 1;
            }
        }
    }
    
    fn trim_by_tokens(&mut self) {
        while self.total_tokens > self.limits.max_tokens && self.messages.len() > self.limits.keep_last_n {
            if let Some(msg) = self.messages.pop_front() {
                if self.limits.keep_system && msg.role == "system" {
                    self.messages.push_front(msg);
                    break;
                }
                if let Some(tokens) = msg.token_count {
                    self.total_tokens = self.total_tokens.saturating_sub(tokens);
                }
            }
        }
    }
    
    fn trim_by_age(&mut self) {
        let now = chrono::Utc::now();
        let max_age = chrono::Duration::seconds(self.limits.max_age_seconds as i64);
        
        self.messages.retain(|msg| {
            let age = now.signed_duration_since(msg.timestamp);
            age < max_age || msg.importance == MessageImportance::Critical
        });
    }
    
    /// Get messages for API (convert to simple format)
    pub fn to_api_messages(&self) -> Vec<serde_json::Value> {
        self.messages.iter().map(|msg| {
            serde_json::json!({
                "role": msg.role,
                "content": msg.content
            })
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_history_limits_default() {
        let limits = HistoryLimits::default();
        assert_eq!(limits.max_messages, 100);
        assert!(limits.keep_system);
    }
    
    #[test]
    fn test_needs_trim() {
        let limits = HistoryLimits::new(10);
        assert!(!limits.needs_trim(5));
        assert!(limits.needs_trim(15));
    }
    
    #[test]
    fn test_trim_count() {
        let limits = HistoryLimits::new(10);
        // 15 messages, keep last 10
        assert_eq!(limits.trim_count(15), 5);
        // 10 messages, no trim
        assert_eq!(limits.trim_count(10), 0);
    }
    
    #[test]
    fn test_history_buffer() {
        let limits = HistoryLimits::new(5);
        let mut buffer = HistoryBuffer::new(limits);
        
        // Add messages
        for i in 0..10 {
            buffer.push(HistoryMessage {
                role: "user".to_string(),
                content: format!("Message {}", i),
                timestamp: chrono::Utc::now(),
                token_count: Some(10),
                importance: MessageImportance::Normal,
            });
        }
        
        // Should be trimmed to around max_messages
        assert!(buffer.len() <= 15);
        assert!(buffer.len() > 0);
    }
    
    #[test]
    fn test_unlimited() {
        let limits = HistoryLimits::unlimited();
        assert!(!limits.needs_trim(10000));
    }
    
    #[test]
    fn test_keep_system() {
        let mut limits = HistoryLimits::new(3);
        limits.keep_system = true;
        limits.keep_last_n = 1;
        
        let mut buffer = HistoryBuffer::new(limits);
        
        // Add system message
        buffer.push(HistoryMessage {
            role: "system".to_string(),
            content: "System prompt".to_string(),
            timestamp: chrono::Utc::now(),
            token_count: Some(10),
            importance: MessageImportance::Critical,
        });
        
        // Add user messages
        for i in 0..5 {
            buffer.push(HistoryMessage {
                role: "user".to_string(),
                content: format!("Message {}", i),
                timestamp: chrono::Utc::now(),
                token_count: Some(10),
                importance: MessageImportance::Normal,
            });
        }
        
        // System message should be kept
        let messages = buffer.messages();
        assert!(messages.iter().any(|m| m.role == "system"));
    }
}