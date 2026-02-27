use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;
use std::path::PathBuf;

pub struct ApplyPatchTool;

#[async_trait]
impl Tool for ApplyPatchTool {
    fn name(&self) -> &str { "apply_patch" }

    fn description(&self) -> &str {
        "Apply a multi-hunk unified diff patch to a file. Provide the patch in unified diff format."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to patch"
                },
                "patch": {
                    "type": "string",
                    "description": "Unified diff patch content"
                }
            },
            "required": ["file_path", "patch"]
        })
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let file_path = params["file_path"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'file_path'"))?;
        let patch = params["patch"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'patch'"))?;

        let path = if PathBuf::from(file_path).is_absolute() {
            PathBuf::from(file_path)
        } else {
            PathBuf::from(&ctx.workspace_dir).join(file_path)
        };

        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", path.display(), e))?;

        let lines: Vec<&str> = content.lines().collect();
        let mut result_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        let mut offset: i64 = 0;

        // Parse unified diff hunks
        for line in patch.lines() {
            if line.starts_with("@@") {
                // Parse hunk header: @@ -start,count +start,count @@
                // Simple approach: apply removals and additions
                continue;
            }
        }

        // Simple patch application: find exact context and apply changes
        // For now, use a line-by-line approach
        let patched = apply_unified_diff(&content, patch)?;

        std::fs::write(&path, &patched)
            .map_err(|e| anyhow::anyhow!("Failed to write {}: {}", path.display(), e))?;

        Ok(ToolResult {
            tool_call_id: String::new(),
            content: format!("Patched {}", path.display()),
            is_error: false,
        })
    }
}

fn apply_unified_diff(original: &str, patch: &str) -> anyhow::Result<String> {
    let orig_lines: Vec<&str> = original.lines().collect();
    let mut result: Vec<String> = Vec::new();
    let mut orig_idx: usize = 0;

    let patch_lines: Vec<&str> = patch.lines().collect();
    let mut hunks: Vec<(usize, Vec<&str>)> = Vec::new();
    let mut current_start: Option<usize> = None;
    let mut current_lines: Vec<&str> = Vec::new();

    for line in &patch_lines {
        if line.starts_with("@@") {
            if let Some(start) = current_start {
                hunks.push((start, std::mem::take(&mut current_lines)));
            }
            // Parse @@ -start,count +start,count @@
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let old_range = parts[1].trim_start_matches('-');
                let start: usize = old_range.split(',').next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1);
                current_start = Some(start.saturating_sub(1)); // 0-indexed
            }
        } else if line.starts_with("---") || line.starts_with("+++") {
            continue;
        } else if current_start.is_some() {
            current_lines.push(line);
        }
    }
    if let Some(start) = current_start {
        hunks.push((start, current_lines));
    }

    for (hunk_start, hunk_lines) in &hunks {
        // Copy lines before this hunk
        while orig_idx < *hunk_start {
            if orig_idx < orig_lines.len() {
                result.push(orig_lines[orig_idx].to_string());
            }
            orig_idx += 1;
        }
        // Apply hunk
        for line in hunk_lines {
            if line.starts_with('-') {
                // Remove line (skip it from original)
                orig_idx += 1;
            } else if line.starts_with('+') {
                // Add line
                result.push(line[1..].to_string());
            } else if line.starts_with(' ') {
                // Context line
                result.push(line[1..].to_string());
                orig_idx += 1;
            } else {
                // Treat as context
                result.push(line.to_string());
                orig_idx += 1;
            }
        }
    }

    // Copy remaining lines
    while orig_idx < orig_lines.len() {
        result.push(orig_lines[orig_idx].to_string());
        orig_idx += 1;
    }

    let mut output = result.join("\n");
    if original.ends_with('\n') {
        output.push('\n');
    }
    Ok(output)
}
