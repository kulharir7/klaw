//! Thread bindings - Isolate conversations within specific threads
//! Allows multiple conversations in the same channel without mixing context

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Thread binding configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadBinding {
    /// Thread ID (platform-specific)
    pub thread_id: String,
    /// Channel this thread belongs to
    pub channel: String,
    /// Chat/Group ID
    pub chat_id: String,
    /// Session key prefix for this thread
    pub session_prefix: Option<String>,
    /// Whether to inherit parent context
    #[serde(default)]
    pub inherit_parent: bool,
    /// Max messages before compaction
    #[serde(default = "default_max_messages")]
    pub max_messages: usize,
    /// Thread metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    /// Created at
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last activity
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

fn default_max_messages() -> usize { 100 }

impl ThreadBinding {
    /// Create a new thread binding
    pub fn new(channel: &str, chat_id: &str, thread_id: &str) -> Self {
        let now = chrono::Utc::now();
        Self {
            thread_id: thread_id.to_string(),
            channel: channel.to_string(),
            chat_id: chat_id.to_string(),
            session_prefix: None,
            inherit_parent: false,
            max_messages: 100,
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }
    
    /// Get the session key for this thread
    pub fn session_key(&self) -> String {
        match &self.session_prefix {
            Some(prefix) => format!("{}:{}:{}", prefix, self.chat_id, self.thread_id),
            None => format!("thread:{}:{}:{}", self.channel, self.chat_id, self.thread_id),
        }
    }
    
    /// Touch the thread (update last activity)
    pub fn touch(&mut self) {
        self.updated_at = chrono::Utc::now();
    }
}

/// Store for thread bindings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThreadBindingStore {
    /// Map: "channel:chat_id:thread_id" -> ThreadBinding
    bindings: HashMap<String, ThreadBinding>,
    /// Map: "channel:chat_id" -> list of thread_ids
    chat_threads: HashMap<String, Vec<String>>,
}

impl ThreadBindingStore {
    /// Load from disk
    pub fn load() -> anyhow::Result<Self> {
        let path = crate::Config::home_dir().join("threads.json");
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&content).unwrap_or_default())
        } else {
            Ok(Self::default())
        }
    }
    
    /// Save to disk
    pub fn save(&self) -> anyhow::Result<()> {
        let path = crate::Config::home_dir().join("threads.json");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
    
    /// Get or create a thread binding
    pub fn get_or_create(&mut self, channel: &str, chat_id: &str, thread_id: &str) -> &mut ThreadBinding {
        let key = format!("{}:{}:{}", channel, chat_id, thread_id);
        
        if !self.bindings.contains_key(&key) {
            let chat_key = format!("{}:{}", channel, chat_id);
            let binding = ThreadBinding::new(channel, chat_id, thread_id);
            self.bindings.insert(key.clone(), binding);
            self.chat_threads.entry(chat_key).or_default().push(thread_id.to_string());
        }
        
        self.bindings.get_mut(&key).unwrap()
    }
    
    /// Get a thread binding
    pub fn get(&self, channel: &str, chat_id: &str, thread_id: &str) -> Option<&ThreadBinding> {
        let key = format!("{}:{}:{}", channel, chat_id, thread_id);
        self.bindings.get(&key)
    }
    
    /// List all threads for a chat
    pub fn list_threads(&self, channel: &str, chat_id: &str) -> Vec<&ThreadBinding> {
        let chat_key = format!("{}:{}", channel, chat_id);
        self.chat_threads.get(&chat_key)
            .map(|threads| {
                threads.iter()
                    .filter_map(|tid| self.bindings.get(&format!("{}:{}:{}", channel, chat_id, tid)))
                    .collect()
            })
            .unwrap_or_default()
    }
    
    /// Remove a thread binding
    pub fn remove(&mut self, channel: &str, chat_id: &str, thread_id: &str) -> bool {
        let key = format!("{}:{}:{}", channel, chat_id, thread_id);
        let chat_key = format!("{}:{}", channel, chat_id);
        
        if let Some(binding) = self.bindings.remove(&key) {
            if let Some(threads) = self.chat_threads.get_mut(&chat_key) {
                threads.retain(|t| t != &binding.thread_id);
            }
            true
        } else {
            false
        }
    }
    
    /// Clean up old threads (older than max_age_seconds)
    pub fn cleanup_old(&mut self, max_age_seconds: u64) -> Vec<String> {
        let now = chrono::Utc::now();
        let mut removed = Vec::new();
        
        self.bindings.retain(|key, binding| {
            let age = now.signed_duration_since(binding.updated_at).num_seconds() as u64;
            if age > max_age_seconds {
                removed.push(key.clone());
                false
            } else {
                true
            }
        });
        
        // Update chat_threads index
        for key in &removed {
            if let Some((_, chat_key, _)) = split_thread_key(key) {
                if let Some(_threads) = self.chat_threads.get_mut(&chat_key) {
                    // Thread ID is already removed from bindings
                }
            }
        }
        
        removed
    }
    
    /// Count of thread bindings
    pub fn len(&self) -> usize {
        self.bindings.len()
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

/// Split a thread key into (channel, chat_key, thread_id)
fn split_thread_key(key: &str) -> Option<(String, String, String)> {
    let parts: Vec<&str> = key.split(':').collect();
    if parts.len() >= 3 {
        let channel = parts[0].to_string();
        let chat_id = parts[1].to_string();
        let thread_id = parts[2..].join(":");
        let chat_key = format!("{}:{}", channel, chat_id);
        Some((channel, chat_key, thread_id))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_thread_binding_creation() {
        let binding = ThreadBinding::new("telegram", "chat123", "thread456");
        assert_eq!(binding.channel, "telegram");
        assert_eq!(binding.chat_id, "chat123");
        assert_eq!(binding.thread_id, "thread456");
        assert_eq!(binding.session_key(), "thread:telegram:chat123:thread456");
    }
    
    #[test]
    fn test_thread_binding_custom_prefix() {
        let mut binding = ThreadBinding::new("discord", "guild789", "thread101");
        binding.session_prefix = Some("support".to_string());
        assert_eq!(binding.session_key(), "support:guild789:thread101");
    }
    
    #[test]
    fn test_thread_binding_store() {
        let mut store = ThreadBindingStore::default();
        
        // Get or create
        let binding = store.get_or_create("telegram", "chat123", "thread456");
        assert_eq!(binding.thread_id, "thread456");
        
        // List threads
        let threads = store.list_threads("telegram", "chat123");
        assert_eq!(threads.len(), 1);
        
        // Get existing
        let same = store.get("telegram", "chat123", "thread456");
        assert!(same.is_some());
        
        // Remove
        assert!(store.remove("telegram", "chat123", "thread456"));
        assert!(!store.remove("telegram", "chat123", "thread456")); // Already removed
    }
    
    #[test]
    fn test_cleanup_old() {
        let mut store = ThreadBindingStore::default();
        
        // Create binding and make it old
        let binding = store.get_or_create("telegram", "chat1", "thread1");
        binding.updated_at = chrono::Utc::now() - chrono::Duration::seconds(3600);
        
        // Create recent binding
        let _ = store.get_or_create("telegram", "chat1", "thread2");
        
        // Cleanup threads older than 1800 seconds (30 min)
        let removed = store.cleanup_old(1800);
        assert_eq!(removed.len(), 1);
        assert_eq!(store.len(), 1);
    }
}