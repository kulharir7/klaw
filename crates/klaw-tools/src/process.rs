use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use tracing::info;

static SESSIONS: std::sync::LazyLock<Mutex<HashMap<String, ProcessSession>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

struct ProcessSession {
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
    finished: bool,
}

/// Add or update a background session (used by exec tool for background mode)
pub fn add_background_session(id: &str, stdout: String, stderr: String, exit_code: Option<i32>, finished: bool) {
    let mut sessions = SESSIONS.lock().unwrap();
    if let Some(existing) = sessions.get_mut(id) {
        if !stdout.is_empty() { existing.stdout = stdout; }
        if !stderr.is_empty() { existing.stderr = stderr; }
        existing.exit_code = exit_code;
        existing.finished = finished;
    } else {
        sessions.insert(id.to_string(), ProcessSession {
            stdout, stderr, exit_code, finished,
        });
    }
}

pub struct ProcessTool;

#[async_trait]
impl Tool for ProcessTool {
    fn name(&self) -> &str { "process" }

    fn description(&self) -> &str {
        "Manage running exec sessions: list, poll, log, write, send-keys, paste, clear, remove, kill."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Process action",
                    "enum": ["list", "poll", "log", "write", "send-keys", "paste", "clear", "remove", "kill"]
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
                "text": {
                    "type": "string",
                    "description": "Text to paste for paste action"
                },
                "keys": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Key tokens to send for send-keys"
                },
                "literal": {
                    "type": "string",
                    "description": "Literal string for send-keys"
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
                let list: Vec<Value> = sessions.iter().map(|(id, s)| {
                    serde_json::json!({
                        "id": id,
                        "finished": s.finished,
                        "exitCode": s.exit_code,
                    })
                }).collect();
                Ok(ToolResult {
                    tool_call_id: String::new(),
                    content: serde_json::to_string_pretty(&list)?,
                    is_error: false,
                })
            }
            "poll" | "log" => {
                let session_id = params["sessionId"].as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'sessionId'"))?;

                // If poll with timeout, wait for completion
                if action == "poll" {
                    if let Some(timeout_ms) = params["timeout"].as_u64() {
                        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
                        loop {
                            {
                                let sessions = SESSIONS.lock().unwrap();
                                if let Some(s) = sessions.get(session_id) {
                                    if s.finished { break; }
                                }
                            }
                            if tokio::time::Instant::now() >= deadline { break; }
                            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                        }
                    }
                }

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
            "write" | "send-keys" | "paste" => {
                let session_id = params["sessionId"].as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'sessionId'"))?;

                let data = match action {
                    "paste" => params["text"].as_str().unwrap_or(""),
                    "send-keys" => params["literal"].as_str().unwrap_or(""),
                    _ => params["data"].as_str().unwrap_or(""),
                };

                // For now, append to session stdout as a record of input sent
                let mut sessions = SESSIONS.lock().unwrap();
                match sessions.get_mut(session_id) {
                    Some(s) => {
                        if s.finished {
                            Ok(ToolResult {
                                tool_call_id: String::new(),
                                content: "Session already finished".to_string(),
                                is_error: true,
                            })
                        } else {
                            info!("process {}: sending data to session {}", action, session_id);
                            // In a real PTY implementation, this would write to the process stdin
                            s.stdout.push_str(&format!("[input: {}]\n", data));
                            Ok(ToolResult {
                                tool_call_id: String::new(),
                                content: format!("Sent {} bytes to session", data.len()),
                                is_error: false,
                            })
                        }
                    }
                    None => Ok(ToolResult {
                        tool_call_id: String::new(),
                        content: format!("Session '{}' not found", session_id),
                        is_error: true,
                    }),
                }
            }
            "clear" => {
                let session_id = params["sessionId"].as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'sessionId'"))?;
                let mut sessions = SESSIONS.lock().unwrap();
                match sessions.get_mut(session_id) {
                    Some(s) => {
                        s.stdout.clear();
                        s.stderr.clear();
                        Ok(ToolResult {
                            tool_call_id: String::new(),
                            content: format!("Session '{}' output cleared", session_id),
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
            "remove" => {
                let session_id = params["sessionId"].as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'sessionId'"))?;
                let mut sessions = SESSIONS.lock().unwrap();
                match sessions.get(session_id) {
                    Some(s) if s.finished => {
                        sessions.remove(session_id);
                        Ok(ToolResult {
                            tool_call_id: String::new(),
                            content: format!("Session '{}' removed", session_id),
                            is_error: false,
                        })
                    }
                    Some(_) => Ok(ToolResult {
                        tool_call_id: String::new(),
                        content: "Cannot remove running session (use kill first)".to_string(),
                        is_error: true,
                    }),
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
