use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;
use std::path::Path;
use tracing::info;

pub struct EditTool;

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str { "edit" }

    fn description(&self) -> &str {
        "Edit a file by replacing exact text. The old text must match exactly (including whitespace). Use this for precise, surgical edits."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to edit"
                },
                "old_string": {
                    "type": "string",
                    "description": "Exact text to find and replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "New text to replace with"
                }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let file_path = params["path"].as_str()
            .or_else(|| params["file_path"].as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'path' parameter"))?;
        let old_string = params["old_string"].as_str()
            .or_else(|| params["oldText"].as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'old_string' parameter"))?;
        let new_string = params["new_string"].as_str()
            .or_else(|| params["newText"].as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'new_string' parameter"))?;

        let path = if Path::new(file_path).is_absolute() {
            file_path.to_string()
        } else {
            format!("{}/{}", ctx.workspace_dir, file_path)
        };

        info!("edit: {} (replacing {} chars)", path, old_string.len());

        if !Path::new(&path).exists() {
            return Ok(ToolResult {
                tool_call_id: String::new(),
                content: format!("File not found: {}", path),
                is_error: true,
            });
        }

        let content = tokio::fs::read_to_string(&path).await?;

        if !content.contains(old_string) {
            return Ok(ToolResult {
                tool_call_id: String::new(),
                content: format!("Old text not found in {}. Make sure it matches exactly (including whitespace).", path),
                is_error: true,
            });
        }

        // Count occurrences
        let count = content.matches(old_string).count();
        let new_content = content.replacen(old_string, new_string, 1);
        tokio::fs::write(&path, &new_content).await?;

        let msg = if count > 1 {
            format!("Replaced first occurrence in {} ({} total matches found)", path, count)
        } else {
            format!("Successfully edited {}", path)
        };

        Ok(ToolResult {
            tool_call_id: String::new(),
            content: msg,
            is_error: false,
        })
    }
}
