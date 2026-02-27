use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;

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
                "message": { "type": "string", "description": "Message text" }
            },
            "required": ["action", "message"]
        })
    }
    async fn execute(&self, _params: Value, _ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        Ok(ToolResult {
            tool_call_id: String::new(),
            content: "Message tool not yet configured.".into(),
            is_error: true,
        })
    }
}
