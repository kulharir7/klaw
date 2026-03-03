//! Tool Permission System
//! 
//! Fine-grained permission control for tools and actions.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Permission level for tools
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionLevel {
    /// No permissions - tool is blocked
    None,
    /// Read-only permissions
    ReadOnly,
    /// Standard permissions (read, write, edit)
    Standard,
    /// Elevated permissions (exec, process, etc.)
    Elevated,
    /// Full permissions (system, critical)
    Full,
}

impl Default for PermissionLevel {
    fn default() -> Self {
        Self::Standard
    }
}

impl PermissionLevel {
    /// Check if level allows the given action
    pub fn allows(&self, action: PermissionLevel) -> bool {
        use PermissionLevel::*;
        match (self, action) {
            (Full, _) => true,
            (Elevated, None | ReadOnly | Standard | Elevated) => true,
            (Standard, None | ReadOnly | Standard) => true,
            (ReadOnly, None | ReadOnly) => true,
            (None, None) => true,
            _ => false,
        }
    }
    
    /// Get numeric value for comparison
    pub fn level(&self) -> u8 {
        match self {
            PermissionLevel::None => 0,
            PermissionLevel::ReadOnly => 1,
            PermissionLevel::Standard => 2,
            PermissionLevel::Elevated => 3,
            PermissionLevel::Full => 4,
        }
    }
}

/// Tool permission configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolPermissions {
    /// Default permission level
    pub default_level: PermissionLevel,
    /// Per-tool permission overrides
    pub tools: HashMap<String, PermissionLevel>,
    /// Per-category permission overrides
    pub categories: HashMap<String, PermissionLevel>,
    /// Blocked tools (always denied)
    pub blocked: HashSet<String>,
    /// Allowed tools (bypass restrictions)
    pub allowed: HashSet<String>,
}

impl Default for ToolPermissions {
    fn default() -> Self {
        let mut categories = HashMap::new();
        categories.insert("read".to_string(), PermissionLevel::ReadOnly);
        categories.insert("write".to_string(), PermissionLevel::Standard);
        categories.insert("exec".to_string(), PermissionLevel::Elevated);
        categories.insert("web".to_string(), PermissionLevel::Standard);
        categories.insert("system".to_string(), PermissionLevel::Full);
        
        Self {
            default_level: PermissionLevel::Standard,
            tools: HashMap::new(),
            categories,
            blocked: HashSet::new(),
            allowed: HashSet::new(),
        }
    }
}

impl ToolPermissions {
    /// Create strict permissions (minimal)
    pub fn strict() -> Self {
        Self {
            default_level: PermissionLevel::ReadOnly,
            tools: HashMap::new(),
            categories: {
                let mut cats = HashMap::new();
                cats.insert("read".to_string(), PermissionLevel::ReadOnly);
                cats
            },
            blocked: HashSet::new(),
            allowed: HashSet::new(),
        }
    }
    
    /// Create full permissions
    pub fn full() -> Self {
        Self {
            default_level: PermissionLevel::Full,
            tools: HashMap::new(),
            categories: {
                let mut cats = HashMap::new();
                cats.insert("read".to_string(), PermissionLevel::Full);
                cats.insert("write".to_string(), PermissionLevel::Full);
                cats.insert("exec".to_string(), PermissionLevel::Full);
                cats.insert("web".to_string(), PermissionLevel::Full);
                cats.insert("system".to_string(), PermissionLevel::Full);
                cats
            },
            blocked: HashSet::new(),
            allowed: HashSet::new(),
        }
    }
    
    /// Check if tool is allowed
    pub fn is_allowed(&self, tool_name: &str, level: PermissionLevel) -> bool {
        // Check blocked first
        if self.blocked.contains(tool_name) {
            return false;
        }
        
        // Check allowed
        if self.allowed.contains(tool_name) {
            return true;
        }
        
        // Check tool-specific
        if let Some(tool_level) = self.tools.get(tool_name) {
            return tool_level.allows(level);
        }
        
        // Check category
        let category = self.get_category(tool_name);
        if let Some(cat_level) = self.categories.get(&category) {
            return cat_level.allows(level);
        }
        
        // Use default
        self.default_level.allows(level)
    }
    
    /// Get category for tool
    fn get_category(&self, tool_name: &str) -> String {
        match tool_name {
            "read" | "web_fetch" | "memory_get" | "memory_search" => "read",
            "write" | "edit" | "apply_patch" | "memory_set" => "write",
            "exec" | "process" | "apply_patch" => "exec",
            "web_search" | "browser" => "web",
            "sessions_spawn" | "sessions_kill" | "system" => "system",
            _ => "standard",
        }.to_string()
    }
    
    /// Block a tool
    pub fn block(&mut self, tool_name: &str) {
        self.blocked.insert(tool_name.to_string());
    }
    
    /// Allow a tool
    pub fn allow(&mut self, tool_name: &str) {
        self.allowed.insert(tool_name.to_string());
    }
    
    /// Set permission level for tool
    pub fn set_tool_level(&mut self, tool_name: &str, level: PermissionLevel) {
        self.tools.insert(tool_name.to_string(), level);
    }
    
    /// Set permission level for category
    pub fn set_category_level(&mut self, category: &str, level: PermissionLevel) {
        self.categories.insert(category.to_string(), level);
    }
    
    /// Merge with another permission set
    pub fn merge(&mut self, other: &ToolPermissions) {
        // Lower default wins
        if other.default_level.level() < self.default_level.level() {
            self.default_level = other.default_level;
        }
        
        // Merge tool levels - lower wins
        for (tool, level) in &other.tools {
            let entry = self.tools.entry(tool.clone()).or_insert(PermissionLevel::Full);
            if level.level() < entry.level() {
                *entry = *level;
            }
        }
        
        // Merge blocked
        for tool in &other.blocked {
            self.blocked.insert(tool.clone());
        }
        
        // Allowed doesn't override blocked
    }
}

/// Permission request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub tool_name: String,
    pub action: String,
    pub level: PermissionLevel,
    pub context: Option<String>,
}

/// Permission manager
pub struct PermissionManager {
    permissions: HashMap<String, ToolPermissions>,
    global_defaults: ToolPermissions,
}

impl PermissionManager {
    pub fn new() -> Self {
        Self {
            permissions: HashMap::new(),
            global_defaults: ToolPermissions::default(),
        }
    }
    
    /// Get permissions for agent
    pub fn get_permissions(&self, agent_id: &str) -> ToolPermissions {
        self.permissions.get(agent_id)
            .cloned()
            .unwrap_or_else(|| self.global_defaults.clone())
    }
    
    /// Set permissions for agent
    pub fn set_permissions(&mut self, agent_id: &str, permissions: ToolPermissions) {
        self.permissions.insert(agent_id.to_string(), permissions);
    }
    
    /// Check if action is allowed
    pub fn is_allowed(&self, agent_id: &str, tool_name: &str, level: PermissionLevel) -> bool {
        let permissions = self.get_permissions(agent_id);
        permissions.is_allowed(tool_name, level)
    }
    
    /// Set global defaults
    pub fn set_global_defaults(&mut self, permissions: ToolPermissions) {
        self.global_defaults = permissions;
    }
}

impl Default for PermissionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_permission_level_allows() {
        assert!(PermissionLevel::Full.allows(PermissionLevel::Elevated));
        assert!(PermissionLevel::Elevated.allows(PermissionLevel::Standard));
        assert!(PermissionLevel::Standard.allows(PermissionLevel::ReadOnly));
        assert!(!PermissionLevel::ReadOnly.allows(PermissionLevel::Standard));
        assert!(!PermissionLevel::None.allows(PermissionLevel::ReadOnly));
    }
    
    #[test]
    fn test_tool_permissions_default() {
        let perms = ToolPermissions::default();
        assert!(perms.is_allowed("read", PermissionLevel::ReadOnly));
        assert!(perms.is_allowed("write", PermissionLevel::Standard));
    }
    
    #[test]
    fn test_tool_permissions_block() {
        let mut perms = ToolPermissions::default();
        perms.block("exec");
        assert!(!perms.is_allowed("exec", PermissionLevel::Full));
    }
    
    #[test]
    fn test_tool_permissions_allow() {
        let mut perms = ToolPermissions::strict();
        perms.allow("write");
        assert!(perms.is_allowed("write", PermissionLevel::Standard));
    }
    
    #[test]
    fn test_permission_manager() {
        let mut manager = PermissionManager::new();
        
        let mut perms = ToolPermissions::strict();
        perms.allow("read");
        manager.set_permissions("agent-1", perms);
        
        assert!(manager.is_allowed("agent-1", "read", PermissionLevel::ReadOnly));
        assert!(!manager.is_allowed("agent-1", "exec", PermissionLevel::Elevated));
    }
    
    #[test]
    fn test_strict_permissions() {
        let perms = ToolPermissions::strict();
        assert!(!perms.is_allowed("write", PermissionLevel::Standard));
        assert!(perms.is_allowed("read", PermissionLevel::ReadOnly));
    }
    
    #[test]
    fn test_full_permissions() {
        let perms = ToolPermissions::full();
        assert!(perms.is_allowed("exec", PermissionLevel::Elevated));
        assert!(perms.is_allowed("system", PermissionLevel::Full));
    }
}