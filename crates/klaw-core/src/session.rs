use crate::config::{Config, DmScope};
use crate::types::{Message, SessionKey};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Session metadata stored in sessions.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub session_id: String,
    pub session_key: String,
    pub agent_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_accessed_at: DateTime<Utc>,
    pub display_name: Option<String>,
    pub channel: Option<String>,
    pub chat_type: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub message_count: u64,
}

/// In-memory session with message history
#[derive(Debug, Clone)]
pub struct Session {
    pub meta: SessionMeta,
    pub messages: Vec<Message>,
    pub locked: bool,
}

impl Session {
    pub fn new(key: &SessionKey, agent_id: &str) -> Self {
        let session_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        Self {
            meta: SessionMeta {
                session_id,
                session_key: key.0.clone(),
                agent_id: agent_id.to_string(),
                created_at: now,
                updated_at: now,
                last_accessed_at: now,
                display_name: None,
                channel: None,
                chat_type: None,
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
                message_count: 0,
            },
            messages: Vec::new(),
            locked: false,
        }
    }

    /// Add a message and update metadata
    pub fn add_message(&mut self, msg: Message) {
        self.meta.message_count += 1;
        self.meta.updated_at = Utc::now();
        self.messages.push(msg);
    }

    /// Update token usage
    pub fn add_usage(&mut self, input: u64, output: u64) {
        self.meta.input_tokens += input;
        self.meta.output_tokens += output;
        self.meta.total_tokens += input + output;
    }

    /// Get recent messages (last N)
    pub fn recent_messages(&self, limit: usize) -> &[Message] {
        let start = self.messages.len().saturating_sub(limit);
        &self.messages[start..]
    }

    /// Prune old messages, keeping last `keep` messages
    pub fn prune(&mut self, keep: usize) {
        if self.messages.len() > keep {
            let drain_count = self.messages.len() - keep;
            self.messages.drain(..drain_count);
        }
    }

    /// Touch the session (update last_accessed_at)
    pub fn touch(&mut self) {
        self.meta.last_accessed_at = Utc::now();
    }

    /// Check if session is idle (no access for `idle_secs` seconds)
    pub fn is_idle(&self, idle_secs: u64) -> bool {
        let now = Utc::now();
        let elapsed = now.signed_duration_since(self.meta.last_accessed_at);
        elapsed.num_seconds() as u64 > idle_secs
    }

    /// Check if session is expired (created more than `max_age_secs` seconds ago)
    pub fn is_expired(&self, max_age_secs: u64) -> bool {
        let now = Utc::now();
        let age = now.signed_duration_since(self.meta.created_at);
        age.num_seconds() as u64 > max_age_secs
    }

    /// Check if session should be reset based on token count
    pub fn needs_token_reset(&self, max_tokens: u64) -> bool {
        self.meta.total_tokens > max_tokens
    }

    /// Reset the session (clear messages and reset metadata)
    pub fn reset(&mut self) {
        let old_created = self.meta.created_at;
        self.messages.clear();
        self.meta = SessionMeta {
            session_id: uuid::Uuid::new_v4().to_string(),
            session_key: self.meta.session_key.clone(),
            agent_id: self.meta.agent_id.clone(),
            created_at: old_created,
            updated_at: Utc::now(),
            last_accessed_at: Utc::now(),
            display_name: None,
            channel: self.meta.channel.clone(),
            chat_type: self.meta.chat_type.clone(),
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            message_count: 0,
        };
    }
}

/// Session store — manages all sessions
pub struct SessionStore {
    sessions: Arc<RwLock<HashMap<String, Session>>>,
    config: Config,
}

impl SessionStore {
    pub fn new(config: Config) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Resolve session key from inbound message context
    pub fn resolve_key(
        &self,
        agent_id: &str,
        channel: &str,
        chat_id: &str,
        sender_id: &str,
        is_group: bool,
    ) -> SessionKey {
        if is_group {
            SessionKey::group(agent_id, channel, chat_id)
        } else {
            match self.config.session.dm_scope {
                DmScope::Main => SessionKey::main(agent_id),
                DmScope::PerPeer => {
                    SessionKey(format!("agent:{}:peer:{}", agent_id, sender_id))
                }
                DmScope::PerChannelPeer => {
                    SessionKey(format!("agent:{}:{}:{}", agent_id, channel, sender_id))
                }
                DmScope::PerAccountChannelPeer => {
                    SessionKey(format!("agent:{}:{}:{}:{}", agent_id, channel, chat_id, sender_id))
                }
            }
        }
    }

    /// Get or create a session
    pub async fn get_or_create(&self, key: &SessionKey, agent_id: &str) -> Session {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get(&key.0) {
            session.clone()
        } else {
            let session = Session::new(key, agent_id);
            sessions.insert(key.0.clone(), session.clone());
            info!("Created new session: {}", key.0);
            session
        }
    }

    /// Update a session (after agent run)
    pub async fn update(&self, session: &Session) {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.meta.session_key.clone(), session.clone());
    }

    /// List all sessions
    pub async fn list(&self) -> Vec<SessionMeta> {
        let sessions = self.sessions.read().await;
        sessions.values().map(|s| s.meta.clone()).collect()
    }

    /// Get session by key
    pub async fn get(&self, key: &str) -> Option<Session> {
        let sessions = self.sessions.read().await;
        sessions.get(key).cloned()
    }

    /// Delete a session
    pub async fn delete(&self, key: &str) -> bool {
        let mut sessions = self.sessions.write().await;
        sessions.remove(key).is_some()
    }

    /// Persist sessions index to disk
    pub async fn save_to_disk(&self, agent_id: &str) -> anyhow::Result<()> {
        let sessions_dir = self.config.sessions_dir(agent_id);
        std::fs::create_dir_all(&sessions_dir)?;

        let index_path = sessions_dir.join("sessions.json");
        let sessions = self.sessions.read().await;
        let metas: HashMap<String, SessionMeta> = sessions
            .iter()
            .map(|(k, s)| (k.clone(), s.meta.clone()))
            .collect();

        let content = serde_json::to_string_pretty(&metas)?;
        std::fs::write(&index_path, content)?;
        info!("Saved {} sessions to {}", metas.len(), index_path.display());
        Ok(())
    }

    /// Load sessions index from disk
    pub async fn load_from_disk(&self, agent_id: &str) -> anyhow::Result<()> {
        let index_path = self.config.sessions_dir(agent_id).join("sessions.json");
        if !index_path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&index_path)?;
        let metas: HashMap<String, SessionMeta> = serde_json::from_str(&content)?;

        let mut sessions = self.sessions.write().await;
        for (key, meta) in metas {
            if !sessions.contains_key(&key) {
                // Load transcript if exists
                let transcript_path = self.config.sessions_dir(agent_id)
                    .join(format!("{}.jsonl", meta.session_id));
                let messages = if transcript_path.exists() {
                    load_transcript(&transcript_path)?
                } else {
                    Vec::new()
                };

                sessions.insert(key, Session {
                    meta,
                    messages,
                    locked: false,
                });
            }
        }
        info!("Loaded {} sessions from disk", sessions.len());
        Ok(())
    }

    /// Append message to JSONL transcript
    pub async fn append_transcript(&self, agent_id: &str, session: &Session, msg: &Message) -> anyhow::Result<()> {
        let sessions_dir = self.config.sessions_dir(agent_id);
        std::fs::create_dir_all(&sessions_dir)?;

        let transcript_path = sessions_dir.join(format!("{}.jsonl", session.meta.session_id));
        let line = serde_json::to_string(msg)? + "\n";

        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&transcript_path)?;
        file.write_all(line.as_bytes())?;
        Ok(())
    }

    /// Clean up idle sessions (not accessed for `idle_secs` seconds)
    pub async fn cleanup_idle(&self, idle_secs: u64) -> Vec<String> {
        let mut sessions = self.sessions.write().await;
        let mut removed = Vec::new();
        
        sessions.retain(|key, session| {
            if session.is_idle(idle_secs) {
                removed.push(key.clone());
                info!("Cleaning up idle session: {}", key);
                false
            } else {
                true
            }
        });

        removed
    }

    /// Clean up expired sessions (older than `max_age_secs` seconds)
    pub async fn cleanup_expired(&self, max_age_secs: u64) -> Vec<String> {
        let mut sessions = self.sessions.write().await;
        let mut removed = Vec::new();
        
        sessions.retain(|key, session| {
            if session.is_expired(max_age_secs) {
                removed.push(key.clone());
                info!("Cleaning up expired session: {}", key);
                false
            } else {
                true
            }
        });

        removed
    }

    /// Reset sessions that exceed token limit
    pub async fn reset_over_token_limit(&self, max_tokens: u64) -> Vec<String> {
        let mut sessions = self.sessions.write().await;
        let mut reset_keys = Vec::new();
        
        for (key, session) in sessions.iter_mut() {
            if session.needs_token_reset(max_tokens) {
                session.reset();
                reset_keys.push(key.clone());
                info!("Reset session due to token limit: {}", key);
            }
        }

        reset_keys
    }

    /// Get session statistics
    pub async fn stats(&self) -> SessionStats {
        let sessions = self.sessions.read().await;
        let total = sessions.len();
        let total_messages: u64 = sessions.values().map(|s| s.meta.message_count).sum();
        let total_tokens: u64 = sessions.values().map(|s| s.meta.total_tokens).sum();
        
        SessionStats {
            total_sessions: total,
            total_messages,
            total_tokens,
        }
    }
}

/// Session statistics
#[derive(Debug, Clone)]
pub struct SessionStats {
    pub total_sessions: usize,
    pub total_messages: u64,
    pub total_tokens: u64,
}

/// Load messages from a JSONL transcript file
fn load_transcript(path: &PathBuf) -> anyhow::Result<Vec<Message>> {
    let content = std::fs::read_to_string(path)?;
    let mut messages = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Message>(line) {
            Ok(msg) => messages.push(msg),
            Err(e) => warn!("Skipping malformed transcript line: {}", e),
        }
    }
    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Role;

    #[tokio::test]
    async fn test_session_create() {
        let config = Config::default();
        let store = SessionStore::new(config);
        let key = SessionKey::main("default");

        let session = store.get_or_create(&key, "default").await;
        assert_eq!(session.meta.session_key, "agent:default:main");
        assert_eq!(session.meta.agent_id, "default");
        assert_eq!(session.messages.len(), 0);
    }

    #[tokio::test]
    async fn test_session_add_message() {
        let config = Config::default();
        let store = SessionStore::new(config);
        let key = SessionKey::main("default");

        let mut session = store.get_or_create(&key, "default").await;
        session.add_message(Message::user("Hello"));
        session.add_message(Message::assistant("Hi there!"));
        store.update(&session).await;

        let loaded = store.get("agent:default:main").await.unwrap();
        assert_eq!(loaded.messages.len(), 2);
        assert_eq!(loaded.meta.message_count, 2);
    }

    #[tokio::test]
    async fn test_session_prune() {
        let config = Config::default();
        let store = SessionStore::new(config);
        let key = SessionKey::main("default");

        let mut session = store.get_or_create(&key, "default").await;
        for i in 0..20 {
            session.add_message(Message::user(&format!("Message {}", i)));
        }
        assert_eq!(session.messages.len(), 20);

        session.prune(10);
        assert_eq!(session.messages.len(), 10);
        // Should keep the last 10
        assert!(session.messages[0].content.contains("10"));
    }

    #[tokio::test]
    async fn test_dm_scope_resolution() {
        let mut config = Config::default();
        let store = SessionStore::new(config.clone());

        // Default: main scope
        let key = store.resolve_key("default", "telegram", "chat123", "user456", false);
        assert_eq!(key.0, "agent:default:main");

        // Group always gets its own key
        let key = store.resolve_key("default", "telegram", "group789", "user456", true);
        assert_eq!(key.0, "agent:default:telegram:group789");

        // Per-channel-peer scope
        config.session.dm_scope = DmScope::PerChannelPeer;
        let store2 = SessionStore::new(config);
        let key = store2.resolve_key("default", "telegram", "chat123", "user456", false);
        assert_eq!(key.0, "agent:default:telegram:user456");
    }

    #[tokio::test]
    async fn test_session_list() {
        let config = Config::default();
        let store = SessionStore::new(config);

        store.get_or_create(&SessionKey::main("default"), "default").await;
        store.get_or_create(&SessionKey::group("default", "telegram", "group1"), "default").await;

        let list = store.list().await;
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_usage_tracking() {
        let config = Config::default();
        let store = SessionStore::new(config);
        let key = SessionKey::main("default");

        let mut session = store.get_or_create(&key, "default").await;
        session.add_usage(100, 50);
        session.add_usage(200, 100);

        assert_eq!(session.meta.input_tokens, 300);
        assert_eq!(session.meta.output_tokens, 150);
        assert_eq!(session.meta.total_tokens, 450);
    }
}
