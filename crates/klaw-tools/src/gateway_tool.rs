use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;

pub struct GatewayTool;

#[async_trait]
impl Tool for GatewayTool {
    fn name(&self) -> &str { "gateway" }
    fn description(&self) -> &str { "Control the gateway daemon." }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["status", "restart"] }
            },
            "required": ["action"]
        })
    }
    async fn execute(&self, _params: Value, _ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        Ok(ToolResult {
            tool_call_id: String::new(),
            content: "Gateway tool not yet configured.".into(),
            is_error: true,
        })
    }
}
