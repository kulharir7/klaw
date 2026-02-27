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
    async fn execute(&self, params: Value, _ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let text = match params["text"].as_str() {
            Some(t) => t,
            None => return Ok(ToolResult {
                tool_call_id: String::new(),
                content: "Missing 'text' parameter.".into(),
                is_error: true,
            }),
        };

        let result = serde_json::json!({
            "text": text,
            "status": "tts_not_configured",
            "supported_providers": ["elevenlabs", "openai", "edge-tts"],
            "note": "Configure a TTS provider in ~/.klaw/klaw.json to enable speech synthesis."
        });

        Ok(ToolResult {
            tool_call_id: String::new(),
            content: serde_json::to_string_pretty(&result).unwrap(),
            is_error: false,
        })
    }
}
