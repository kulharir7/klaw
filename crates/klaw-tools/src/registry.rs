use crate::Tool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Registry of all available tools
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.get(name)
    }

    /// List all tool names
    pub fn list(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    /// Generate tool schemas for LLM (OpenAI format)
    pub fn tool_schemas(&self) -> Vec<serde_json::Value> {
        self.tools
            .values()
            .map(|tool| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool.name(),
                        "description": tool.description(),
                        "parameters": tool.parameters_schema(),
                    }
                })
            })
            .collect()
    }

    /// Tool count
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Generate filtered tool schemas based on profile, allow, and deny lists.
    /// Groups are expanded, profile provides base set, allow intersects, deny subtracts (deny wins).
    pub fn filtered_tool_schemas(
        &self,
        profile: Option<&str>,
        allow: Option<&[String]>,
        deny: Option<&[String]>,
    ) -> Vec<serde_json::Value> {
        let all_names: HashSet<String> = self.tools.keys().cloned().collect();

        // Start with profile base set
        let mut active = match profile {
            Some("minimal") => expand_list(&["session_status"]),
            Some("coding") => expand_list(&[
                "group:fs", "group:runtime", "group:sessions", "group:memory", "image",
            ]),
            Some("messaging") => expand_list(&[
                "group:messaging", "sessions_list", "sessions_history",
                "sessions_send", "session_status",
            ]),
            Some("full") | None => all_names.clone(),
            Some(_) => all_names.clone(),
        };

        // Intersect with allow list if provided
        if let Some(allow_list) = allow {
            let allowed = expand_strings(allow_list);
            active = active.intersection(&allowed).cloned().collect();
        }

        // Subtract deny list (deny always wins)
        if let Some(deny_list) = deny {
            let denied = expand_strings(deny_list);
            active = active.difference(&denied).cloned().collect();
        }

        // Only include tools that actually exist
        active = active.intersection(&all_names).cloned().collect();

        self.tools
            .values()
            .filter(|tool| active.contains(tool.name()))
            .map(|tool| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool.name(),
                        "description": tool.description(),
                        "parameters": tool.parameters_schema(),
                    }
                })
            })
            .collect()
    }
}

/// Tool group definitions
fn group_tools(group: &str) -> Vec<&'static str> {
    match group {
        "group:runtime" => vec!["exec", "process"],
        "group:fs" => vec!["read", "write", "edit", "apply_patch"],
        "group:sessions" => vec![
            "sessions_list", "sessions_history", "sessions_send",
            "sessions_spawn", "session_status",
        ],
        "group:memory" => vec!["memory_search", "memory_get"],
        "group:web" => vec!["web_search", "web_fetch"],
        "group:ui" => vec!["browser", "canvas"],
        "group:automation" => vec!["cron", "gateway"],
        "group:messaging" => vec!["message"],
        "group:nodes" => vec!["nodes"],
        _ => vec![],
    }
}

/// Expand a slice of strings, resolving group: prefixes
fn expand_strings(items: &[String]) -> HashSet<String> {
    let mut result = HashSet::new();
    for item in items {
        let lower = item.to_lowercase();
        if lower.starts_with("group:") {
            for t in group_tools(&lower) {
                result.insert(t.to_string());
            }
        } else {
            result.insert(lower);
        }
    }
    result
}

/// Expand a slice of &str, resolving group: prefixes
fn expand_list(items: &[&str]) -> HashSet<String> {
    let strings: Vec<String> = items.iter().map(|s| s.to_string()).collect();
    expand_strings(&strings)
}
