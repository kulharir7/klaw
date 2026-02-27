use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;

pub struct CanvasTool;

#[async_trait]
impl Tool for CanvasTool {
    fn name(&self) -> &str { "canvas" }
    fn description(&self) -> &str { "Control node canvases (present/hide/navigate/eval/snapshot)." }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["present", "hide", "navigate", "eval", "snapshot"] },
                "url": { "type": "string" }
            },
            "required": ["action"]
        })
    }
    async fn execute(&self, _params: Value, _ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        Ok(ToolResult {
            tool_call_id: String::new(),
            content: "Canvas tool not yet configured.".into(),
            is_error: true,
        })
    }
}
