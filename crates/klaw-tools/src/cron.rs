use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;

pub struct CronTool;

#[async_trait]
impl Tool for CronTool {
    fn name(&self) -> &str { "cron" }
    fn description(&self) -> &str { "Schedule recurring tasks." }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["list", "add", "remove"] },
                "schedule": { "type": "string", "description": "Cron expression" },
                "task": { "type": "string", "description": "Task description" }
            },
            "required": ["action"]
        })
    }
    async fn execute(&self, _params: Value, _ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        Ok(ToolResult {
            tool_call_id: String::new(),
            content: "Cron tool not yet configured.".into(),
            is_error: true,
        })
    }
}
