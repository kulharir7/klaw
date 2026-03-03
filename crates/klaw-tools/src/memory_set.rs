//! Memory Set Tool
//! 
//! Write to agent memory files for persistence across sessions.

use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::fs;
use chrono::Utc;

use crate::{Tool, ToolContext};

pub struct MemorySetTool;

impl MemorySetTool {
    pub fn new() -> Self {
        Self
    }
    
    fn get_memory_dir(&self, ctx: &ToolContext) -> PathBuf {
        PathBuf::from(&ctx.workspace_dir)
            .join(".memory")
            .join(&ctx.agent_id)
    }
    
    fn get_memory_file(&self, ctx: &ToolContext, filename: &str) -> PathBuf {
        self.get_memory_dir(ctx).join(filename)
    }
}

#[async_trait]
impl Tool for MemorySetTool {
    fn name(&self) -> &str {
        "memory_set"
    }
    
    fn description(&self) -> &str {
        "Write to agent memory file. Creates or appends to memory files in .memory/{agent_id}/ directory. Use for persisting information across sessions."
    }
    
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "filename": {
                    "type": "string",
                    "description": "Memory filename (e.g., '2026-03-03.md' or 'project-notes.md')"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to memory file"
                },
                "mode": {
                    "type": "string",
                    "enum": ["overwrite", "append", "prepend"],
                    "default": "append",
                    "description": "Write mode: 'overwrite' replaces, 'append' adds to end, 'prepend' adds to start"
                },
                "timestamp": {
                    "type": "boolean",
                    "default": true,
                    "description": "Add timestamp header"
                }
            },
            "required": ["filename", "content"]
        })
    }
    
    async fn execute(&self, params: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let filename = params.get("filename")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("filename required"))?;
        
        let content = params.get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("content required"))?;
        
        let mode = params.get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("append");
        
        let add_timestamp = params.get("timestamp")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        
        // Ensure memory directory exists
        let memory_dir = self.get_memory_dir(ctx);
        fs::create_dir_all(&memory_dir).await
            .map_err(|e| anyhow::anyhow!("Failed to create memory dir: {}", e))?;
        
        let file_path = self.get_memory_file(ctx, filename);
        
        // Prepare content based on mode
        let final_content = match mode {
            "overwrite" => {
                if add_timestamp {
                    format!("# Memory - {}\n\n{}\n", 
                        Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
                        content
                    )
                } else {
                    content.to_string()
                }
            }
            "prepend" => {
                let header = if add_timestamp {
                    format!("\n---\n\n## {} - {}\n\n", 
                        Utc::now().format("%Y-%m-%d"),
                        Utc::now().format("%H:%M:%S UTC")
                    )
                } else {
                    "\n".to_string()
                };
                
                // Read existing if present
                let existing = if file_path.exists() {
                    fs::read_to_string(&file_path).await.unwrap_or_default()
                } else {
                    String::new()
                };
                
                format!("{}{}{}", header, content, existing)
            }
            "append" | _ => {
                let header = if add_timestamp {
                    format!("\n---\n\n## {} - {}\n\n", 
                        Utc::now().format("%Y-%m-%d"),
                        Utc::now().format("%H:%M:%S UTC")
                    )
                } else {
                    "\n\n".to_string()
                };
                
                // Read existing if present
                let existing = if file_path.exists() {
                    fs::read_to_string(&file_path).await.unwrap_or_default()
                } else {
                    format!("# Memory - {}\n\n", Utc::now().format("%Y-%m-%d"))
                };
                
                format!("{}{}{}", existing, header, content)
            }
        };
        
        // Write file
        fs::write(&file_path, &final_content).await
            .map_err(|e| anyhow::anyhow!("Failed to write memory: {}", e))?;
        
        let relative_path = file_path.strip_prefix(&ctx.workspace_dir)
            .unwrap_or(&file_path);
        
        Ok(ToolResult {
            tool_call_id: String::new(),
            content: serde_json::to_string_pretty(&json!({
                "success": true,
                "file": filename,
                "path": relative_path.to_string_lossy(),
                "mode": mode,
                "bytes_written": final_content.len(),
                "message": format!("Memory saved to {}", filename)
            })).unwrap(),
            is_error: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_memory_set_create() {
        // Create a temp directory manually
        let temp_dir = std::env::temp_dir();
        let test_dir = temp_dir.join("klaw_test_memory");
        let _ = tokio::fs::create_dir_all(&test_dir).await;
        
        let ctx = ToolContext {
            workspace_dir: test_dir.to_string_lossy().to_string(),
            session_key: "test-session".into(),
            agent_id: "test-agent".into(),
        };
        
        let tool = MemorySetTool::new();
        let result = tool.execute(json!({
            "filename": "test.md",
            "content": "Test memory content"
        }), &ctx).await.unwrap();
        
        assert!(!result.is_error);
        assert!(result.content.contains("success"));
        
        // Cleanup
        let _ = tokio::fs::remove_dir_all(&test_dir).await;
    }
    
    #[tokio::test]
    async fn test_memory_set_overwrite() {
        let temp_dir = std::env::temp_dir();
        let test_dir = temp_dir.join("klaw_test_memory2");
        let _ = tokio::fs::create_dir_all(&test_dir).await;
        
        let ctx = ToolContext {
            workspace_dir: test_dir.to_string_lossy().to_string(),
            session_key: "test-session".into(),
            agent_id: "test-agent".into(),
        };
        
        let tool = MemorySetTool::new();
        
        // First write
        tool.execute(json!({
            "filename": "test.md",
            "content": "First content",
            "mode": "overwrite"
        }), &ctx).await.unwrap();
        
        // Overwrite
        let result = tool.execute(json!({
            "filename": "test.md",
            "content": "Second content",
            "mode": "overwrite"
        }), &ctx).await.unwrap();
        
        assert!(!result.is_error);
        
        // Cleanup
        let _ = tokio::fs::remove_dir_all(&test_dir).await;
    }
}