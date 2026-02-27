use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;
use chrono::Local;
use std::path::PathBuf;

fn klaw_home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".klaw")
}

fn agents_dir() -> PathBuf {
    klaw_home().join("agents")
}

fn ok(content: String) -> ToolResult {
    ToolResult { tool_call_id: String::new(), content, is_error: false }
}

fn err(content: String) -> ToolResult {
    ToolResult { tool_call_id: String::new(), content, is_error: true }
}

// --- SessionStatus ---
pub struct SessionStatus;

#[async_trait]
impl Tool for SessionStatus {
    fn name(&self) -> &str { "session_status" }
    fn description(&self) -> &str { "Get current session status and metadata." }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({ "type": "object", "properties": {} })
    }
    async fn execute(&self, _params: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let now = Local::now();
        let content = format!(
            "📊 Session Status\n\
             ─────────────────\n\
             Date/Time: {}\n\
             Day: {}\n\
             Session Key: {}\n\
             Agent ID: {}\n\
             Workspace: {}\n\
             Klaw Home: {}",
            now.format("%Y-%m-%d %H:%M:%S %Z"),
            now.format("%A"),
            ctx.session_key,
            ctx.agent_id,
            ctx.workspace_dir,
            klaw_home().display()
        );
        Ok(ok(content))
    }
}

// --- SessionsList ---
pub struct SessionsList;

#[async_trait]
impl Tool for SessionsList {
    fn name(&self) -> &str { "sessions_list" }
    fn description(&self) -> &str { "List active sessions." }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "recentMinutes": { "type": "number", "description": "Filter to sessions active in last N minutes" }
            }
        })
    }
    async fn execute(&self, _params: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let sessions_file = agents_dir()
            .join(&ctx.agent_id)
            .join("sessions")
            .join("sessions.json");

        if !sessions_file.exists() {
            return Ok(ok("No sessions found. Sessions index does not exist yet.".into()));
        }

        match tokio::fs::read_to_string(&sessions_file).await {
            Ok(content) => Ok(ok(format!("Sessions for agent '{}':\n{}", ctx.agent_id, content))),
            Err(e) => Ok(err(format!("Failed to read sessions index: {}", e))),
        }
    }
}

// --- SessionsHistory ---
pub struct SessionsHistory;

#[async_trait]
impl Tool for SessionsHistory {
    fn name(&self) -> &str { "sessions_history" }
    fn description(&self) -> &str { "Get message history for a session." }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "sessionKey": { "type": "string", "description": "Session key" },
                "limit": { "type": "number", "description": "Max messages to return (default 20)" }
            },
            "required": ["sessionKey"]
        })
    }
    async fn execute(&self, params: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let session_key = match params["sessionKey"].as_str() {
            Some(k) => k,
            None => return Ok(err("Missing 'sessionKey' parameter.".into())),
        };
        let limit = params["limit"].as_u64().unwrap_or(20) as usize;

        let transcript = agents_dir()
            .join(&ctx.agent_id)
            .join("sessions")
            .join(format!("{}.jsonl", session_key));

        if !transcript.exists() {
            return Ok(err(format!("No transcript found for session '{}'.", session_key)));
        }

        match tokio::fs::read_to_string(&transcript).await {
            Ok(content) => {
                let lines: Vec<&str> = content.lines().collect();
                let start = if lines.len() > limit { lines.len() - limit } else { 0 };
                let selected = &lines[start..];
                Ok(ok(format!("Last {} messages for session '{}':\n{}", selected.len(), session_key, selected.join("\n"))))
            }
            Err(e) => Ok(err(format!("Failed to read transcript: {}", e))),
        }
    }
}

// --- AgentsList ---
pub struct AgentsList;

#[async_trait]
impl Tool for AgentsList {
    fn name(&self) -> &str { "agents_list" }
    fn description(&self) -> &str { "List configured agents." }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({ "type": "object", "properties": {} })
    }
    async fn execute(&self, _params: Value, _ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let dir = agents_dir();
        if !dir.exists() {
            return Ok(ok("No agents directory found. No agents configured yet.".into()));
        }

        let mut agents = Vec::new();
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    agents.push(name.to_string());
                }
            }
        }

        if agents.is_empty() {
            Ok(ok("No agents found.".into()))
        } else {
            Ok(ok(format!("Agents ({}):\n{}", agents.len(), agents.iter().map(|a| format!("  • {}", a)).collect::<Vec<_>>().join("\n"))))
        }
    }
}

// --- SessionsSend ---
pub struct SessionsSend;

#[async_trait]
impl Tool for SessionsSend {
    fn name(&self) -> &str { "sessions_send" }
    fn description(&self) -> &str { "Send a message to a session." }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "sessionKey": { "type": "string" },
                "message": { "type": "string" }
            },
            "required": ["sessionKey", "message"]
        })
    }
    async fn execute(&self, params: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let session_key = match params["sessionKey"].as_str() {
            Some(k) => k,
            None => return Ok(err("Missing 'sessionKey' parameter.".into())),
        };
        let message = match params["message"].as_str() {
            Some(m) => m,
            None => return Ok(err("Missing 'message' parameter.".into())),
        };

        let sessions_dir = agents_dir()
            .join(&ctx.agent_id)
            .join("sessions");
        std::fs::create_dir_all(&sessions_dir).ok();

        let transcript = sessions_dir.join(format!("{}.jsonl", session_key));
        let now = Local::now().to_rfc3339();
        let entry = serde_json::json!({
            "timestamp": now,
            "role": "injected",
            "content": message,
            "from_session": ctx.session_key
        });

        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&transcript)?;
        writeln!(file, "{}", entry)?;

        Ok(ok(format!("Message sent to session '{}'.", session_key)))
    }
}

// --- SessionsSpawn ---
pub struct SessionsSpawn;

#[async_trait]
impl Tool for SessionsSpawn {
    fn name(&self) -> &str { "sessions_spawn" }
    fn description(&self) -> &str { "Spawn a new sub-agent session." }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task": { "type": "string", "description": "Task description for the sub-agent" },
                "model": { "type": "string" }
            },
            "required": ["task"]
        })
    }
    async fn execute(&self, _params: Value, _ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        Ok(ok("Sub-agent spawning requires gateway integration. Use sessions_send for basic cross-session messaging.".into()))
    }
}
