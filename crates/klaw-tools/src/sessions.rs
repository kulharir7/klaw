use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;

macro_rules! stub_tool {
    ($struct_name:ident, $name:expr, $desc:expr, $schema:expr) => {
        pub struct $struct_name;

        #[async_trait]
        impl Tool for $struct_name {
            fn name(&self) -> &str { $name }
            fn description(&self) -> &str { $desc }
            fn parameters_schema(&self) -> Value { $schema }
            async fn execute(&self, _params: Value, _ctx: &ToolContext) -> anyhow::Result<ToolResult> {
                Ok(ToolResult {
                    tool_call_id: String::new(),
                    content: format!("{} not yet implemented.", $name),
                    is_error: true,
                })
            }
        }
    };
}

stub_tool!(SessionsList, "sessions_list", "List active sessions.", serde_json::json!({
    "type": "object",
    "properties": {
        "recentMinutes": { "type": "number", "description": "Filter to sessions active in last N minutes" }
    }
}));

stub_tool!(SessionsHistory, "sessions_history", "Get message history for a session.", serde_json::json!({
    "type": "object",
    "properties": {
        "sessionKey": { "type": "string", "description": "Session key" },
        "limit": { "type": "number" }
    },
    "required": ["sessionKey"]
}));

stub_tool!(SessionsSend, "sessions_send", "Send a message to a session.", serde_json::json!({
    "type": "object",
    "properties": {
        "sessionKey": { "type": "string" },
        "message": { "type": "string" }
    },
    "required": ["sessionKey", "message"]
}));

stub_tool!(SessionsSpawn, "sessions_spawn", "Spawn a new sub-agent session.", serde_json::json!({
    "type": "object",
    "properties": {
        "task": { "type": "string", "description": "Task description for the sub-agent" },
        "model": { "type": "string" }
    },
    "required": ["task"]
}));

stub_tool!(SessionStatus, "session_status", "Get current session status and metadata.", serde_json::json!({
    "type": "object",
    "properties": {}
}));

stub_tool!(AgentsList, "agents_list", "List configured agents.", serde_json::json!({
    "type": "object",
    "properties": {}
}));
