use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;
use chrono::Local;
use std::path::PathBuf;

fn klaw_home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".klaw")
}

pub struct MessageTool;

#[async_trait]
impl Tool for MessageTool {
    fn name(&self) -> &str { "message" }
    fn description(&self) -> &str { "Send messages via channel plugins." }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["send"] },
                "target": { "type": "string", "description": "Target channel/user" },
                "message": { "type": "string", "description": "Message text" },
                "channel": { "type": "string", "description": "Channel plugin (default: webchat)" }
            },
            "required": ["action", "message"]
        })
    }
    async fn execute(&self, params: Value, _ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let action = params["action"].as_str().unwrap_or("send");
        if action != "send" {
            return Ok(ToolResult {
                tool_call_id: String::new(),
                content: format!("Unknown action: '{}'. Only 'send' is supported.", action),
                is_error: true,
            });
        }

        let message = match params["message"].as_str() {
            Some(m) => m,
            None => return Ok(ToolResult {
                tool_call_id: String::new(),
                content: "Missing 'message' parameter.".into(),
                is_error: true,
            }),
        };
        let target = params["target"].as_str().unwrap_or("default");
        let channel = params["channel"].as_str().unwrap_or("webchat");

        let outbox_dir = klaw_home().join("outbox");
        std::fs::create_dir_all(&outbox_dir).ok();

        let outbox_file = outbox_dir.join(format!("{}_{}.jsonl", channel, target));
        let entry = serde_json::json!({
            "timestamp": Local::now().to_rfc3339(),
            "target": target,
            "message": message,
            "channel": channel,
            "status": "queued"
        });

        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&outbox_file)?;
        writeln!(file, "{}", entry)?;

        Ok(ToolResult {
            tool_call_id: String::new(),
            content: format!("Message queued for delivery to {} via {}.", target, channel),
            is_error: false,
        })
    }
}
