use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;
use std::path::PathBuf;

fn klaw_home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".klaw")
}

fn canvas_dir() -> PathBuf {
    klaw_home().join("canvas")
}

fn ok(content: String) -> ToolResult {
    ToolResult { tool_call_id: String::new(), content, is_error: false }
}

fn err(content: String) -> ToolResult {
    ToolResult { tool_call_id: String::new(), content, is_error: true }
}

pub struct CanvasTool;

#[async_trait]
impl Tool for CanvasTool {
    fn name(&self) -> &str { "canvas" }
    fn description(&self) -> &str { "Control node canvases (present/hide/navigate/eval/snapshot/a2ui_push/a2ui_reset)." }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["present", "hide", "navigate", "eval", "snapshot", "a2ui_push", "a2ui_reset"] },
                "url": { "type": "string" },
                "javaScript": { "type": "string" },
                "jsonl": { "type": "string", "description": "JSONL content for a2ui_push" }
            },
            "required": ["action"]
        })
    }
    async fn execute(&self, params: Value, _ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let action = params["action"].as_str().unwrap_or("present");
        let dir = canvas_dir();

        match action {
            "present" => {
                let url = params["url"].as_str().unwrap_or("");
                std::fs::create_dir_all(&dir).ok();
                let html = if url.is_empty() {
                    "<html><body><h1>Canvas</h1></body></html>".to_string()
                } else {
                    format!("<html><body><iframe src=\"{}\" style=\"width:100%;height:100vh;border:none\"></iframe></body></html>", url)
                };
                match std::fs::write(dir.join("current.html"), &html) {
                    Ok(_) => Ok(ok("Canvas presented.".into())),
                    Err(e) => Ok(err(format!("Failed to write canvas: {}", e))),
                }
            }
            "hide" => {
                let path = dir.join("current.html");
                if path.exists() {
                    std::fs::remove_file(&path).ok();
                }
                Ok(ok("Canvas hidden.".into()))
            }
            "eval" => {
                Ok(ok("Canvas eval requires an active browser/node connection. JavaScript evaluation is not available in file-based mode.".into()))
            }
            "snapshot" => {
                Ok(ok("Canvas snapshot requires node connection.".into()))
            }
            "navigate" => {
                let url = params["url"].as_str().unwrap_or("");
                Ok(ok(format!("Canvas navigate to '{}' requires node connection.", url)))
            }
            "a2ui_push" => {
                let jsonl = params["jsonl"].as_str().unwrap_or("");
                if jsonl.is_empty() {
                    return Ok(err("Missing 'jsonl' parameter.".into()));
                }
                std::fs::create_dir_all(&dir).ok();
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(dir.join("a2ui.jsonl"))?;
                writeln!(file, "{}", jsonl)?;
                Ok(ok("A2UI content pushed.".into()))
            }
            "a2ui_reset" => {
                let path = dir.join("a2ui.jsonl");
                if path.exists() {
                    std::fs::remove_file(&path).ok();
                }
                Ok(ok("A2UI reset.".into()))
            }
            _ => Ok(err(format!("Unknown canvas action: '{}'", action))),
        }
    }
}
