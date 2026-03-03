//! Binding resolver - routes messages to agents based on match rules

use crate::config::{Binding, BindingMatch, Config};
use serde_json::Value;
use std::collections::HashMap;

/// Resolves which agent should handle an incoming message
#[derive(Debug, Clone)]
pub struct BindingResolver {
    bindings: Vec<Binding>,
    default_agent: String,
}

impl BindingResolver {
    /// Create a new resolver from config
    pub fn from_config(config: &Config) -> Self {
        let bindings = config.bindings.clone().unwrap_or_default();
        // Default to "default" agent, or use first agent in list if available
        let default_agent = config.agents.list.first()
            .map(|a| a.id.clone())
            .unwrap_or_else(|| "default".to_string());
        
        Self { bindings, default_agent }
    }
    
    /// Resolve agent for a channel message
    pub fn resolve(&self, ctx: &MessageContext) -> String {
        for binding in &self.bindings {
            if self.matches(&binding.match_rule, ctx) {
                return binding.agent_id.clone();
            }
        }
        self.default_agent.clone()
    }
    
    /// Check if a binding matches the message context
    fn matches(&self, rule: &BindingMatch, ctx: &MessageContext) -> bool {
        // Channel match
        if let Some(ref channel) = rule.channel {
            if !Self::match_pattern(channel, &ctx.channel) {
                return false;
            }
        }
        
        // Account ID match (bot/user ID)
        if let Some(ref account_id) = rule.account_id {
            if !Self::match_pattern(account_id, &ctx.account_id) {
                return false;
            }
        }
        
        // Guild ID match (Discord servers)
        if let Some(ref guild_id) = rule.guild_id {
            if !Self::match_pattern(guild_id, &ctx.guild_id) {
                return false;
            }
        }
        
        // Team ID match (Slack teams)
        if let Some(ref team_id) = rule.team_id {
            if !Self::match_pattern(team_id, &ctx.team_id) {
                return false;
            }
        }
        
        // Peer match (complex matching)
        if let Some(ref peer) = rule.peer {
            if !self.match_peer(peer, ctx) {
                return false;
            }
        }
        
        true
    }
    
    /// Match a glob pattern against a value
    fn match_pattern(pattern: &str, value: &str) -> bool {
        if pattern == "*" || pattern == value {
            return true;
        }
        
        // Handle glob patterns like "telegram-*"
        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];
                return value.starts_with(prefix) && value.ends_with(suffix);
            }
        }
        
        // Handle comma-separated values
        if pattern.contains(',') {
            return pattern.split(',')
                .map(|s| s.trim())
                .any(|s| s == value);
        }
        
        pattern == value
    }
    
    /// Match peer rules (complex peer matching)
    fn match_peer(&self, peer: &Value, ctx: &MessageContext) -> bool {
        match peer {
            Value::String(s) => Self::match_pattern(s, &ctx.peer_id),
            Value::Array(arr) => arr.iter().any(|v| self.match_peer(v, ctx)),
            Value::Object(obj) => {
                // Check peer properties
                if let Some(id) = obj.get("id") {
                    if let Some(id_str) = id.as_str() {
                        if !Self::match_pattern(id_str, &ctx.peer_id) {
                            return false;
                        }
                    }
                }
                if let Some(name) = obj.get("name") {
                    if let Some(name_str) = name.as_str() {
                        if !Self::match_pattern(name_str, &ctx.peer_name) {
                            return false;
                        }
                    }
                }
                true
            }
            _ => false,
        }
    }
    
    /// Add a binding at runtime
    pub fn add_binding(&mut self, binding: Binding) {
        self.bindings.push(binding);
    }
    
    /// Remove a binding by agent_id
    pub fn remove_binding(&mut self, agent_id: &str) -> bool {
        let len_before = self.bindings.len();
        self.bindings.retain(|b| b.agent_id != agent_id);
        self.bindings.len() < len_before
    }
    
    /// List all bindings
    pub fn list_bindings(&self) -> &[Binding] {
        &self.bindings
    }
}

/// Context for resolving which agent should handle a message
#[derive(Debug, Clone, Default)]
pub struct MessageContext {
    /// Channel type (telegram, discord, slack, webchat, etc.)
    pub channel: String,
    /// Account ID (bot ID, user ID)
    pub account_id: String,
    /// Peer ID (user/chat who sent the message)
    pub peer_id: String,
    /// Peer name (display name)
    pub peer_name: String,
    /// Guild ID (Discord server)
    pub guild_id: String,
    /// Team ID (Slack team)
    pub team_id: String,
    /// Thread ID (if in a thread)
    pub thread_id: String,
    /// Message content for pattern matching
    pub content: String,
    /// Additional metadata
    pub metadata: HashMap<String, Value>,
}

impl MessageContext {
    /// Create a context for a channel message
    pub fn new(channel: impl Into<String>, peer_id: impl Into<String>) -> Self {
        Self {
            channel: channel.into(),
            peer_id: peer_id.into(),
            ..Default::default()
        }
    }
    
    /// Set account ID
    pub fn account(mut self, id: impl Into<String>) -> Self {
        self.account_id = id.into();
        self
    }
    
    /// Set peer name
    pub fn peer_name(mut self, name: impl Into<String>) -> Self {
        self.peer_name = name.into();
        self
    }
    
    /// Set guild ID
    pub fn guild(mut self, id: impl Into<String>) -> Self {
        self.guild_id = id.into();
        self
    }
    
    /// Set team ID
    pub fn team(mut self, id: impl Into<String>) -> Self {
        self.team_id = id.into();
        self
    }
    
    /// Set thread ID
    pub fn thread(mut self, id: impl Into<String>) -> Self {
        self.thread_id = id.into();
        self
    }
    
    /// Set message content
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }
    
    /// Add metadata
    pub fn meta(mut self, key: impl Into<String>, value: Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic_binding() {
        let mut config = Config::default();
        config.bindings = Some(vec![
            Binding {
                agent_id: "discord-bot".to_string(),
                match_rule: BindingMatch {
                    channel: Some("discord".to_string()),
                    ..Default::default()
                },
            },
            Binding {
                agent_id: "telegram-bot".to_string(),
                match_rule: BindingMatch {
                    channel: Some("telegram".to_string()),
                    ..Default::default()
                },
            },
        ]);
        
        let resolver = BindingResolver::from_config(&config);
        
        let ctx = MessageContext::new("discord", "user123");
        assert_eq!(resolver.resolve(&ctx), "discord-bot");
        
        let ctx = MessageContext::new("telegram", "user456");
        assert_eq!(resolver.resolve(&ctx), "telegram-bot");
        
        let ctx = MessageContext::new("webchat", "user789");
        assert_eq!(resolver.resolve(&ctx), "default");
    }
    
    #[test]
    fn test_guild_binding() {
        let mut config = Config::default();
        config.bindings = Some(vec![
            Binding {
                agent_id: "guild-bot".to_string(),
                match_rule: BindingMatch {
                    channel: Some("discord".to_string()),
                    guild_id: Some("123456789".to_string()),
                    ..Default::default()
                },
            },
        ]);
        
        let resolver = BindingResolver::from_config(&config);
        
        let ctx = MessageContext::new("discord", "user123").guild("123456789");
        assert_eq!(resolver.resolve(&ctx), "guild-bot");
        
        let ctx = MessageContext::new("discord", "user123").guild("987654321");
        assert_eq!(resolver.resolve(&ctx), "default");
    }
    
    #[test]
    fn test_glob_pattern() {
        let mut config = Config::default();
        config.bindings = Some(vec![
            Binding {
                agent_id: "all-discord".to_string(),
                match_rule: BindingMatch {
                    channel: Some("discord-*".to_string()),
                    ..Default::default()
                },
            },
        ]);
        
        let resolver = BindingResolver::from_config(&config);
        
        // This would match if we had channels like "discord-main", "discord-alt"
        // For now, test exact match
        let ctx = MessageContext::new("discord", "user123");
        assert_eq!(resolver.resolve(&ctx), "default");
    }
}