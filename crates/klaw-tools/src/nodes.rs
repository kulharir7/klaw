use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;

pub struct NodesTool;

#[async_trait]
impl Tool for NodesTool {
    fn name(&self) -> &str { "nodes" }
    fn description(&self) -> &str { "Discover and control paired nodes." }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["status", "describe", "notify", "run"] },
                "node": { "type": "string" }
            },
            "required": ["action"]
        })
    }
    async fn execute(&self, _params: Value, _ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        Ok(ToolResult {
            tool_call_id: String::new(),
            content: "Nodes tool not yet configured.".into(),
            is_error: true,
        })
    }
}
