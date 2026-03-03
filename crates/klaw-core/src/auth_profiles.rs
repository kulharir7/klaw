//! Auth profile management with key rotation and cooldowns

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Auth profile with multiple API keys and rotation settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthProfile {
    /// Profile name
    pub name: String,
    /// List of API keys
    pub keys: Vec<AuthKey>,
    /// Rotation mode: round_robin, random, cooldown
    #[serde(default = "default_rotation_mode")]
    pub rotation_mode: String,
    /// Cooldown duration in seconds (for cooldown mode)
    #[serde(default = "default_cooldown")]
    pub cooldown_seconds: u64,
    /// Session stickiness - use same key for session duration
    #[serde(default)]
    pub session_sticky: bool,
}

fn default_rotation_mode() -> String { "round_robin".to_string() }
fn default_cooldown() -> u64 { 60 }

/// Single API key with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthKey {
    /// The API key value
    pub key: String,
    /// Optional label for this key
    pub label: Option<String>,
    /// Key weight for weighted distribution (default: 1)
    #[serde(default = "default_weight")]
    pub weight: u32,
    /// Whether this key is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_weight() -> u32 { 1 }
fn default_enabled() -> bool { true }

/// Runtime state for key rotation
#[derive(Debug, Clone)]
pub struct AuthProfileState {
    /// Current key index (for round_robin)
    pub current_index: usize,
    /// Last used key index per session
    pub session_keys: HashMap<String, usize>,
    /// Last use time per key index
    pub last_used: Vec<Option<Instant>>,
    /// Error count per key index
    pub error_counts: Vec<u32>,
    /// Keys that are temporarily disabled
    pub disabled_until: Vec<Option<Instant>>,
}

impl AuthProfileState {
    pub fn new(key_count: usize) -> Self {
        Self {
            current_index: 0,
            session_keys: HashMap::new(),
            last_used: vec![None; key_count],
            error_counts: vec![0; key_count],
            disabled_until: vec![None; key_count],
        }
    }
}

/// Manager for auth profiles
#[derive(Debug, Clone)]
pub struct AuthProfileManager {
    profiles: HashMap<String, AuthProfile>,
    states: HashMap<String, AuthProfileState>,
}

impl AuthProfileManager {
    /// Create a new manager
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
            states: HashMap::new(),
        }
    }
    
    /// Add or update a profile
    pub fn add_profile(&mut self, profile: AuthProfile) {
        let key_count = profile.keys.len();
        self.states.insert(profile.name.clone(), AuthProfileState::new(key_count));
        self.profiles.insert(profile.name.clone(), profile);
    }
    
    /// Get a profile by name
    pub fn get_profile(&self, name: &str) -> Option<&AuthProfile> {
        self.profiles.get(name)
    }
    
    /// Get the next key to use
    pub fn get_next_key(&mut self, profile_name: &str, session_id: Option<&str>) -> Option<String> {
        let profile = self.profiles.get(profile_name)?;
        let state = self.states.get_mut(profile_name)?;
        
        if profile.keys.is_empty() {
            return None;
        }
        
        // Get enabled keys
        let enabled_indices: Vec<usize> = profile.keys.iter()
            .enumerate()
            .filter(|(_, k)| k.enabled)
            .map(|(i, _)| i)
            .collect();
        
        if enabled_indices.is_empty() {
            return None;
        }
        
        // Filter out keys in cooldown
        let now = Instant::now();
        let available_indices: Vec<usize> = enabled_indices.into_iter()
            .filter(|&i| {
                if let Some(Some(disabled_until)) = state.disabled_until.get(i) {
                    now < *disabled_until
                } else {
                    true
                }
            })
            .collect();
        
        if available_indices.is_empty() {
            // All keys in cooldown, use the one with least wait time
            // Need to find it before we have a mutable borrow
            let now = Instant::now();
            let mut best_idx = 0;
            let mut best_wait = Duration::from_secs(u64::MAX);
            
            for (i, key) in profile.keys.iter().enumerate() {
                if !key.enabled {
                    continue;
                }
                if let Some(Some(disabled_until)) = state.disabled_until.get(i) {
                    if now < *disabled_until {
                        let wait = disabled_until.duration_since(now);
                        if wait < best_wait {
                            best_wait = wait;
                            best_idx = i;
                        }
                    }
                }
            }
            
            return profile.keys.get(best_idx).map(|k| k.key.clone());
        }
        
        // Check session stickiness
        if profile.session_sticky {
            if let Some(sid) = session_id {
                if let Some(&idx) = state.session_keys.get(sid) {
                    if let Some(key) = profile.keys.get(idx) {
                        if key.enabled {
                            return Some(key.key.clone());
                        }
                    }
                }
            }
        }
        
        // Select key based on rotation mode
        let selected_idx = match profile.rotation_mode.as_str() {
            "round_robin" => {
                let idx = available_indices.iter()
                    .find(|&&i| i >= state.current_index)
                    .copied()
                    .unwrap_or_else(|| available_indices[0]);
                state.current_index = idx + 1;
                if state.current_index >= profile.keys.len() {
                    state.current_index = 0;
                }
                idx
            }
            "random" => {
                use rand::Rng;
                let mut rng = rand::thread_rng();
                available_indices[rng.gen_range(0..available_indices.len())]
            }
            "cooldown" => {
                // Select the key not used for longest time
                available_indices.into_iter()
                    .filter(|&i| profile.keys.get(i).map(|k| k.enabled).unwrap_or(false))
                    .min_by_key(|&i| state.last_used.get(i).and_then(|t| *t).unwrap_or(Instant::now() - Duration::from_secs(3600)))
                    .unwrap_or(0)
            }
            _ => {
                // Default to round_robin
                let idx = state.current_index;
                state.current_index = (idx + 1) % profile.keys.len();
                idx
            }
        };
        
        // Update state
        if let Some(last_used) = state.last_used.get_mut(selected_idx) {
            *last_used = Some(now);
        }
        
        if let Some(sid) = session_id {
            state.session_keys.insert(sid.to_string(), selected_idx);
        }
        
        profile.keys.get(selected_idx).map(|k| k.key.clone())
    }
    
    /// Report an error with a key (triggers cooldown)
    pub fn report_error(&mut self, profile_name: &str, key: &str, error_type: &str) {
        let profile = match self.profiles.get(profile_name) {
            Some(p) => p,
            None => return,
        };
        let state = match self.states.get_mut(profile_name) {
            Some(s) => s,
            None => return,
        };
        
        // Find the key index
        let key_idx = profile.keys.iter().position(|k| k.key == key);
        if let Some(idx) = key_idx {
            // Increment error count
            if let Some(count) = state.error_counts.get_mut(idx) {
                *count += 1;
            }
            
            // Apply cooldown based on error type
            let cooldown_multiplier = match error_type {
                "rate_limit" => 2,
                "insufficient_credits" => 10,
                "auth_error" => 5,
                _ => 1,
            };
            
            let cooldown_seconds = profile.cooldown_seconds * cooldown_multiplier;
            let disabled_until = Instant::now() + Duration::from_secs(cooldown_seconds);
            
            if let Some(slot) = state.disabled_until.get_mut(idx) {
                *slot = Some(disabled_until);
            }
        }
    }
    
    /// Clear cooldown for a key
    pub fn clear_cooldown(&mut self, profile_name: &str, key: &str) {
        let profile = match self.profiles.get(profile_name) {
            Some(p) => p,
            None => return,
        };
        let state = match self.states.get_mut(profile_name) {
            Some(s) => s,
            None => return,
        };
        
        if let Some(idx) = profile.keys.iter().position(|k| k.key == key) {
            if let Some(slot) = state.disabled_until.get_mut(idx) {
                *slot = None;
            }
        }
    }
    
    /// Get profile status
    pub fn get_status(&self, profile_name: &str) -> Option<AuthProfileStatus> {
        let profile = self.profiles.get(profile_name)?;
        let state = self.states.get(profile_name)?;
        
        let now = Instant::now();
        let keys_status: Vec<KeyStatus> = profile.keys.iter().enumerate()
            .map(|(i, k)| {
                let in_cooldown = state.disabled_until.get(i)
                    .and_then(|d| d.map(|d| now < d))
                    .unwrap_or(false);
                let error_count = state.error_counts.get(i).copied().unwrap_or(0);
                
                KeyStatus {
                    label: k.label.clone(),
                    enabled: k.enabled,
                    in_cooldown,
                    error_count,
                }
            })
            .collect();
        
        Some(AuthProfileStatus {
            name: profile.name.clone(),
            rotation_mode: profile.rotation_mode.clone(),
            total_keys: profile.keys.len(),
            enabled_keys: keys_status.iter().filter(|k| k.enabled).count(),
            keys_in_cooldown: keys_status.iter().filter(|k| k.in_cooldown).count(),
            keys: keys_status,
        })
    }
    
    /// List all profile names
    pub fn list_profiles(&self) -> Vec<&str> {
        self.profiles.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for AuthProfileManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Status of an auth profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthProfileStatus {
    pub name: String,
    pub rotation_mode: String,
    pub total_keys: usize,
    pub enabled_keys: usize,
    pub keys_in_cooldown: usize,
    pub keys: Vec<KeyStatus>,
}

/// Status of a single key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyStatus {
    pub label: Option<String>,
    pub enabled: bool,
    pub in_cooldown: bool,
    pub error_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_round_robin_rotation() {
        let profile = AuthProfile {
            name: "test".to_string(),
            keys: vec![
                AuthKey { key: "key1".to_string(), label: None, weight: 1, enabled: true },
                AuthKey { key: "key2".to_string(), label: None, weight: 1, enabled: true },
                AuthKey { key: "key3".to_string(), label: None, weight: 1, enabled: true },
            ],
            rotation_mode: "round_robin".to_string(),
            cooldown_seconds: 60,
            session_sticky: false,
        };
        
        let mut manager = AuthProfileManager::new();
        manager.add_profile(profile);
        
        assert_eq!(manager.get_next_key("test", None), Some("key1".to_string()));
        assert_eq!(manager.get_next_key("test", None), Some("key2".to_string()));
        assert_eq!(manager.get_next_key("test", None), Some("key3".to_string()));
        assert_eq!(manager.get_next_key("test", None), Some("key1".to_string()));
    }
    
    #[test]
    fn test_session_sticky() {
        let profile = AuthProfile {
            name: "sticky".to_string(),
            keys: vec![
                AuthKey { key: "key1".to_string(), label: None, weight: 1, enabled: true },
                AuthKey { key: "key2".to_string(), label: None, weight: 1, enabled: true },
            ],
            rotation_mode: "round_robin".to_string(),
            cooldown_seconds: 60,
            session_sticky: true,
        };
        
        let mut manager = AuthProfileManager::new();
        manager.add_profile(profile);
        
        // First call for session1 should get key1
        let key1 = manager.get_next_key("sticky", Some("session1"));
        assert_eq!(key1, Some("key1".to_string()));
        
        // Session1 should always get key1
        assert_eq!(manager.get_next_key("sticky", Some("session1")), Some("key1".to_string()));
        assert_eq!(manager.get_next_key("sticky", Some("session1")), Some("key1".to_string()));
        
        // Session2 should get key2 (next in rotation)
        assert_eq!(manager.get_next_key("sticky", Some("session2")), Some("key2".to_string()));
    }
    
    #[test]
    fn test_error_cooldown() {
        let profile = AuthProfile {
            name: "cooldown".to_string(),
            keys: vec![
                AuthKey { key: "key1".to_string(), label: None, weight: 1, enabled: true },
                AuthKey { key: "key2".to_string(), label: None, weight: 1, enabled: true },
            ],
            rotation_mode: "round_robin".to_string(),
            cooldown_seconds: 60,
            session_sticky: false,
        };
        
        let mut manager = AuthProfileManager::new();
        manager.add_profile(profile);
        
        // Report rate limit error for key1
        manager.report_error("cooldown", "key1", "rate_limit");
        
        let status = manager.get_status("cooldown").unwrap();
        assert_eq!(status.keys_in_cooldown, 1);
        assert!(status.keys[0].in_cooldown);
    }
}