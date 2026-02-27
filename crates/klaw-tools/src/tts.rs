use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;

pub struct TtsTool;

#[async_trait]
impl Tool for TtsTool {
    fn name(&self) -> &str { "tts" }
    fn description(&self) -> &str { "Convert text to speech." }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "Text to convert to speech" }
            },
            "required": ["text"]
        })
    }
    async fn execute(&self, _params: Value, _ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        Ok(ToolResult {
            tool_call_id: String::new(),
            content: "TTS not yet configured.".into(),
            is_error: true,
        })
    }
}
