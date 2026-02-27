use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;
use std::path::PathBuf;

pub struct MemorySearchTool;

#[async_trait]
impl Tool for MemorySearchTool {
    fn name(&self) -> &str { "memory_search" }

    fn description(&self) -> &str {
        "Search through MEMORY.md and memory/*.md files for keywords. Returns matching lines with context."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query (case-insensitive keyword search)"
                },
                "limit": {
                    "type": "number",
                    "description": "Max results to return (default 20)"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let query = params["query"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'query'"))?;
        let limit = params["limit"].as_u64().unwrap_or(20) as usize;
        let query_lower = query.to_lowercase();

        let ws = PathBuf::from(&ctx.workspace_dir);
        let mut results: Vec<String> = Vec::new();

        // Search MEMORY.md
        let memory_file = ws.join("MEMORY.md");
        if memory_file.exists() {
            search_file(&memory_file, &query_lower, &mut results, limit);
        }

        // Search memory/*.md
        let memory_dir = ws.join("memory");
        if memory_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&memory_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("md") {
                        search_file(&path, &query_lower, &mut results, limit);
                        if results.len() >= limit { break; }
                    }
                }
            }
        }

        let content = if results.is_empty() {
            format!("No matches found for '{}'", query)
        } else {
            results.truncate(limit);
            results.join("\n")
        };

        Ok(ToolResult {
            tool_call_id: String::new(),
            content,
            is_error: false,
        })
    }
}

fn search_file(path: &PathBuf, query: &str, results: &mut Vec<String>, limit: usize) {
    if results.len() >= limit { return; }
    if let Ok(content) = std::fs::read_to_string(path) {
        let filename = path.file_name().and_then(|f| f.to_str()).unwrap_or("?");
        for (i, line) in content.lines().enumerate() {
            if line.to_lowercase().contains(query) {
                results.push(format!("{}:{}: {}", filename, i + 1, line));
                if results.len() >= limit { return; }
            }
        }
    }
}
