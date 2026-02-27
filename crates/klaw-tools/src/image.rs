use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;

pub struct ImageTool;

#[async_trait]
impl Tool for ImageTool {
    fn name(&self) -> &str { "image" }
    fn description(&self) -> &str { "Analyze images with a vision model." }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "image": { "type": "string", "description": "Image path or URL" },
                "prompt": { "type": "string", "description": "Analysis prompt" }
            }
        })
    }
    async fn execute(&self, _params: Value, _ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        Ok(ToolResult {
            tool_call_id: String::new(),
            content: "Image analysis not yet configured.".into(),
            is_error: true,
        })
    }
}
