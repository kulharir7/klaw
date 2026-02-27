use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::process::Command;
use tracing::info;

static SESSIONS: std::sync::LazyLock<Mutex<HashMap<String, ProcessSession>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

struct ProcessSession {
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
    finished: bool,
}

pub struct ProcessTool;

#[async_trait]
impl Tool for ProcessTool {
    fn name(&self) -> &str { "process" }

    fn description(&self) -> &str {
        "Manage running exec sessions: list, poll, log, write, kill."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Process action: list, poll, log, write, kill",
                    "enum": ["list", "poll", "log", "write", "kill"]
                },
                "sessionId": {
                    "type": "string",
                    "description": "Session id for actions other than list"
                },
                "timeout": {
                    "type": "number",
                    "description": "For poll: wait up to this many milliseconds"
                },
                "data": {
                    "type": "string",
                    "description": "Data to write for write action"
                },
                "limit": { "type": "number", "description": "Log length" },
                "offset": { "type": "number", "description": "Log offset" }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let action = params["action"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'action' parameter"))?;

        match action {
            "list" => {
                let sessions = SESSIONS.lock().unwrap();
                let list: Vec<_> = sessions.keys().cloned().collect();
                Ok(ToolResult {
                    tool_call_id: String::new(),
                    content: serde_json::to_string_pretty(&list)?,
                    is_error: false,
                })
            }
            "poll" | "log" => {
                let session_id = params["sessionId"].as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'sessionId'"))?;
                let sessions = SESSIONS.lock().unwrap();
                match sessions.get(session_id) {
                    Some(s) => {
                        let offset = params["offset"].as_u64().unwrap_or(0) as usize;
                        let limit = params["limit"].as_u64().unwrap_or(200) as usize;
                        let lines: Vec<&str> = s.stdout.lines().skip(offset).take(limit).collect();
                        Ok(ToolResult {
                            tool_call_id: String::new(),
                            content: serde_json::json!({
                                "stdout": lines.join("\n"),
                                "stderr": s.stderr,
                                "finished": s.finished,
                                "exitCode": s.exit_code,
                            }).to_string(),
                            is_error: false,
                        })
                    }
                    None => Ok(ToolResult {
                        tool_call_id: String::new(),
                        content: format!("Session '{}' not found", session_id),
                        is_error: true,
                    }),
                }
            }
            "kill" => {
                let session_id = params["sessionId"].as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'sessionId'"))?;
                let mut sessions = SESSIONS.lock().unwrap();
                if sessions.remove(session_id).is_some() {
                    Ok(ToolResult {
                        tool_call_id: String::new(),
                        content: format!("Session '{}' killed", session_id),
                        is_error: false,
                    })
                } else {
                    Ok(ToolResult {
                        tool_call_id: String::new(),
                        content: format!("Session '{}' not found", session_id),
                        is_error: true,
                    })
                }
            }
            _ => Ok(ToolResult {
                tool_call_id: String::new(),
                content: format!("Unknown action: {}", action),
                is_error: true,
            }),
        }
    }
}
