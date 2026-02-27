use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;
use std::path::PathBuf;

pub struct MemoryGetTool;

#[async_trait]
impl Tool for MemoryGetTool {
    fn name(&self) -> &str { "memory_get" }

    fn description(&self) -> &str {
        "Read a memory file snippet. Reads from MEMORY.md or memory/<file>.md with optional line range."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file": {
                    "type": "string",
                    "description": "File name (e.g. 'MEMORY.md' or '2025-01-15.md'). Defaults to MEMORY.md"
                },
                "from": {
                    "type": "number",
                    "description": "Start line (1-indexed, default 1)"
                },
                "lines": {
                    "type": "number",
                    "description": "Number of lines to read (default 100)"
                }
            }
        })
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let file = params["file"].as_str().unwrap_or("MEMORY.md");
        let from = params["from"].as_u64().unwrap_or(1).max(1) as usize;
        let lines = params["lines"].as_u64().unwrap_or(100) as usize;

        let ws = PathBuf::from(&ctx.workspace_dir);
        let path = if file == "MEMORY.md" {
            ws.join(file)
        } else {
            ws.join("memory").join(file)
        };

        if !path.exists() {
            return Ok(ToolResult {
                tool_call_id: String::new(),
                content: format!("File not found: {}", file),
                is_error: true,
            });
        }

        let content = std::fs::read_to_string(&path)?;
        let selected: Vec<&str> = content.lines()
            .skip(from - 1)
            .take(lines)
            .collect();

        let total_lines = content.lines().count();

        Ok(ToolResult {
            tool_call_id: String::new(),
            content: format!(
                "{}  (lines {}-{} of {})\n{}",
                file, from, (from + selected.len()).saturating_sub(1), total_lines,
                selected.join("\n")
            ),
            is_error: false,
        })
    }
}
