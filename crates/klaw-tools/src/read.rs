use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;
use std::path::Path;
use tracing::info;

pub struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str { "read" }

    fn description(&self) -> &str {
        "Read the contents of a file. Supports text files. Output is truncated for large files. Use offset/limit for pagination."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read (relative or absolute)"
                },
                "offset": {
                    "type": "number",
                    "description": "Line number to start reading from (1-indexed)"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of lines to read"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let file_path = params["path"].as_str()
            .or_else(|| params["file_path"].as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'path' parameter"))?;

        let offset = params["offset"].as_u64().unwrap_or(1).max(1) as usize;
        let limit = params["limit"].as_u64().unwrap_or(2000) as usize;

        // Resolve relative paths against workspace
        let path = if Path::new(file_path).is_absolute() {
            file_path.to_string()
        } else {
            format!("{}/{}", ctx.workspace_dir, file_path)
        };

        info!("read: {} (offset: {}, limit: {})", path, offset, limit);

        if !Path::new(&path).exists() {
            return Ok(ToolResult {
                tool_call_id: String::new(),
                content: format!("File not found: {}", path),
                is_error: true,
            });
        }

        let content = tokio::fs::read_to_string(&path).await?;
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        let start = (offset - 1).min(total_lines);
        let end = (start + limit).min(total_lines);
        let selected: Vec<&str> = lines[start..end].to_vec();
        let result = selected.join("\n");

        // Truncate to 50KB
        let result = if result.len() > 50000 {
            format!("{}...\n(truncated at 50KB, file has {} lines)", &result[..49000], total_lines)
        } else if end < total_lines {
            format!("{}\n\n[{} more lines in file. Use offset={} to continue.]", result, total_lines - end, end + 1)
        } else {
            result
        };

        Ok(ToolResult {
            tool_call_id: String::new(),
            content: result,
            is_error: false,
        })
    }
}
