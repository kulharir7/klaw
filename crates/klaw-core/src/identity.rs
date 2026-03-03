//! Cross-channel identity mapping
//! Maps users across different channels (Telegram, Discord, Slack, etc.)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Unique identity across all channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    /// Unique identity ID
    pub id: String,
    /// Display name (chosen by user or derived)
    pub display_name: Option<String>,
    /// Linked channels
    pub links: Vec<IdentityLink>,
    /// Metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Created at
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Updated at
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// A link to a specific channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityLink {
    /// Channel type (telegram, discord, slack, webchat, etc.)
    pub channel: String,
    /// User ID in that channel
    pub user_id: String,
    /// Display name in that channel
    pub display_name: Option<String>,
    /// Channel-specific metadata
    pub metadata: HashMap<String, String>,
    /// When this link was verified
    pub verified_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Identity store for cross-channel mapping
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IdentityStore {
    /// Map: identity_id -> Identity
    identities: HashMap<String, Identity>,
    /// Map: "channel:user_id" -> identity_id
    link_index: HashMap<String, String>,
    /// Path to store file
    #[serde(skip)]
    path: PathBuf,
}

impl IdentityStore {
    /// Load identity store from disk
    pub fn load() -> anyhow::Result<Self> {
        let path = crate::Config::home_dir().join("identities.json");
        Self::load_from(&path)
    }
    
    /// Load from a specific path
    pub fn load_from(path: &PathBuf) -> anyhow::Result<Self> {
        let (identities, link_index) = if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let identities: HashMap<String, Identity> = serde_json::from_str(&content)
                .unwrap_or_default();
            
            // Build link index
            let mut link_index = HashMap::new();
            for (id, identity) in &identities {
                for link in &identity.links {
                    let key = format!("{}:{}", link.channel, link.user_id);
                    link_index.insert(key, id.clone());
                }
            }
            
            (identities, link_index)
        } else {
            (HashMap::new(), HashMap::new())
        };
        
        Ok(Self {
            identities,
            link_index,
            path: path.clone(),
        })
    }
    
    /// Save identity store to disk
    pub fn save(&self) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&self.identities)?;
        std::fs::write(&self.path, content)?;
        Ok(())
    }
    
    /// Get identity by ID
    pub fn get(&self, identity_id: &str) -> Option<&Identity> {
        self.identities.get(identity_id)
    }
    
    /// Find identity by channel and user ID
    pub fn find_by_channel(&self, channel: &str, user_id: &str) -> Option<&Identity> {
        let key = format!("{}:{}", channel, user_id);
        self.link_index.get(&key)
            .and_then(|id| self.identities.get(id))
    }
    
    /// Find identity ID by channel and user ID
    pub fn find_identity_id(&self, channel: &str, user_id: &str) -> Option<&str> {
        let key = format!("{}:{}", channel, user_id);
        self.link_index.get(&key).map(|s| s.as_str())
    }
    
    /// Create a new identity
    pub fn create(&mut self, display_name: Option<String>) -> Identity {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        Identity {
            id: id.clone(),
            display_name,
            links: Vec::new(),
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }
    
    /// Add or update an identity
    pub fn upsert(&mut self, identity: Identity) {
        // Update link index
        for link in &identity.links {
            let key = format!("{}:{}", link.channel, link.user_id);
            self.link_index.insert(key, identity.id.clone());
        }
        
        self.identities.insert(identity.id.clone(), identity);
    }
    
    /// Link a channel to an identity
    pub fn link(&mut self, identity_id: &str, link: IdentityLink) -> anyhow::Result<()> {
        let identity = self.identities.get_mut(identity_id)
            .ok_or_else(|| anyhow::anyhow!("Identity not found: {}", identity_id))?;
        
        // Check if link already exists
        let existing = identity.links.iter_mut()
            .find(|l| l.channel == link.channel && l.user_id == link.user_id);
        
        if let Some(existing) = existing {
            // Update existing link
            existing.display_name = link.display_name;
            existing.verified_at = link.verified_at;
        } else {
            // Add new link
            let key = format!("{}:{}", link.channel, link.user_id);
            self.link_index.insert(key, identity_id.to_string());
            identity.links.push(link);
        }
        
        identity.updated_at = chrono::Utc::now();
        Ok(())
    }
    
    /// Unlink a channel from an identity
    pub fn unlink(&mut self, identity_id: &str, channel: &str, user_id: &str) -> anyhow::Result<()> {
        let identity = self.identities.get_mut(identity_id)
            .ok_or_else(|| anyhow::anyhow!("Identity not found: {}", identity_id))?;
        
        let key = format!("{}:{}", channel, user_id);
        self.link_index.remove(&key);
        
        identity.links.retain(|l| !(l.channel == channel && l.user_id == user_id));
        identity.updated_at = chrono::Utc::now();
        
        Ok(())
    }
    
    /// Get all identities
    pub fn list(&self) -> Vec<&Identity> {
        self.identities.values().collect()
    }
    
    /// Get or create identity for a channel user
    pub fn get_or_create(&mut self, channel: &str, user_id: &str, display_name: Option<String>) -> String {
        // Check if already linked
        let existing_id = self.find_identity_id(channel, user_id).map(|s| s.to_string());
        
        if let Some(identity_id) = existing_id {
            // Update display name if provided
            if let Some(name) = display_name {
                if let Some(identity) = self.identities.get_mut(&identity_id) {
                    if identity.display_name.is_none() {
                        identity.display_name = Some(name);
                        identity.updated_at = chrono::Utc::now();
                    }
                }
            }
            return identity_id;
        }
        
        // Create new identity
        let mut identity = self.create(display_name.clone());
        
        // Add link
        let link = IdentityLink {
            channel: channel.to_string(),
            user_id: user_id.to_string(),
            display_name,
            metadata: HashMap::new(),
            verified_at: Some(chrono::Utc::now()),
        };
        identity.links.push(link);
        
        let id = identity.id.clone();
        self.upsert(identity);
        
        id
    }
    
    /// Merge two identities (move all links from source to target)
    pub fn merge(&mut self, source_id: &str, target_id: &str) -> anyhow::Result<()> {
        let source_links = {
            let source = self.identities.get(source_id)
                .ok_or_else(|| anyhow::anyhow!("Source identity not found: {}", source_id))?;
            source.links.clone()
        };
        
        for link in source_links {
            self.link(target_id, link)?;
        }
        
        // Remove source identity
        self.identities.remove(source_id);
        
        Ok(())
    }
    
    /// Delete an identity
    pub fn delete(&mut self, identity_id: &str) -> bool {
        if let Some(identity) = self.identities.remove(identity_id) {
            // Remove from link index
            for link in &identity.links {
                let key = format!("{}:{}", link.channel, link.user_id);
                self.link_index.remove(&key);
            }
            true
        } else {
            false
        }
    }
    
    /// Count of identities
    pub fn len(&self) -> usize {
        self.identities.len()
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.identities.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_and_link() {
        let mut store = IdentityStore::default();
        
        let identity_id = store.get_or_create("telegram", "12345", Some("Alice".to_string()));
        
        assert!(store.get(&identity_id).is_some());
        assert_eq!(store.find_identity_id("telegram", "12345"), Some(identity_id.as_str()));
    }
    
    #[test]
    fn test_cross_channel_link() {
        let mut store = IdentityStore::default();
        
        // Create identity with Telegram link
        let identity_id = store.get_or_create("telegram", "12345", Some("Alice".to_string()));
        
        // Link Discord account
        let discord_link = IdentityLink {
            channel: "discord".to_string(),
            user_id: "67890".to_string(),
            display_name: Some("Alice#1234".to_string()),
            metadata: HashMap::new(),
            verified_at: Some(chrono::Utc::now()),
        };
        store.link(&identity_id, discord_link).unwrap();
        
        // Both should resolve to same identity
        assert_eq!(store.find_identity_id("telegram", "12345"), Some(identity_id.as_str()));
        assert_eq!(store.find_identity_id("discord", "67890"), Some(identity_id.as_str()));
    }
    
    #[test]
    fn test_unlink() {
        let mut store = IdentityStore::default();
        
        let identity_id = store.get_or_create("telegram", "12345", Some("Alice".to_string()));
        
        let discord_link = IdentityLink {
            channel: "discord".to_string(),
            user_id: "67890".to_string(),
            display_name: None,
            metadata: HashMap::new(),
            verified_at: None,
        };
        store.link(&identity_id, discord_link).unwrap();
        
        // Unlink Discord
        store.unlink(&identity_id, "discord", "67890").unwrap();
        
        // Discord should no longer resolve
        assert!(store.find_identity_id("discord", "67890").is_none());
        // Telegram should still work
        assert_eq!(store.find_identity_id("telegram", "12345"), Some(identity_id.as_str()));
    }
    
    #[test]
    fn test_merge_identities() {
        let mut store = IdentityStore::default();
        
        // Create two separate identities
        let id1 = store.get_or_create("telegram", "12345", Some("Alice".to_string()));
        let id2 = store.get_or_create("discord", "67890", Some("Alice#1234".to_string()));
        
        // Merge id1 into id2
        store.merge(&id1, &id2).unwrap();
        
        // Both channels should now resolve to id2
        assert_eq!(store.find_identity_id("telegram", "12345"), Some(id2.as_str()));
        assert_eq!(store.find_identity_id("discord", "67890"), Some(id2.as_str()));
        
        // id1 should no longer exist
        assert!(store.get(&id1).is_none());
    }
}