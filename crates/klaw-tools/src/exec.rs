use crate::{Tool, ToolContext};
use crate::process::add_background_session;
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;
use tokio::process::Command;
use tracing::{info, warn};

pub struct ExecTool;

#[async_trait]
impl Tool for ExecTool {
    fn name(&self) -> &str { "exec" }

    fn description(&self) -> &str {
        "Execute shell commands with background continuation. Use yieldMs/background to continue later via process tool. Use pty=true for TTY-required commands."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command to execute"
                },
                "workdir": {
                    "type": "string",
                    "description": "Working directory (defaults to workspace)"
                },
                "timeout": {
                    "type": "number",
                    "description": "Timeout in seconds (default 30)"
                },
                "pty": {
                    "type": "boolean",
                    "description": "Run in a pseudo-terminal (PTY) when available"
                },
                "elevated": {
                    "type": "boolean",
                    "description": "Run with elevated permissions (if allowed)"
                },
                "host": {
                    "type": "string",
                    "description": "Exec host: sandbox, gateway, or node",
                    "enum": ["sandbox", "gateway", "node"]
                },
                "security": {
                    "type": "string",
                    "description": "Exec security mode",
                    "enum": ["deny", "allowlist", "full"]
                },
                "background": {
                    "type": "boolean",
                    "description": "Run in background immediately, return session_id"
                },
                "yieldMs": {
                    "type": "number",
                    "description": "Milliseconds to wait before backgrounding (default 10000)"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, params: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let command = params["command"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'command' parameter"))?;
        let workdir = params["workdir"].as_str()
            .unwrap_or(&ctx.workspace_dir);
        let timeout_secs = params["timeout"].as_u64().unwrap_or(30);

        // Handle PTY request (log only for now)
        let pty = params["pty"].as_bool().unwrap_or(false);
        if pty {
            info!("exec: PTY requested (not yet implemented, running normally)");
        }

        // Handle elevated
        let elevated = params["elevated"].as_bool().unwrap_or(false);
        if elevated {
            return Ok(ToolResult {
                tool_call_id: String::new(),
                content: "Elevated execution is not enabled in this configuration.".to_string(),
                is_error: true,
            });
        }

        // Handle host
        let host = params["host"].as_str().unwrap_or("gateway");
        info!("exec: host={}", host);

        // Handle security mode
        let security = params["security"].as_str().unwrap_or("full");
        if security == "deny" {
            return Ok(ToolResult {
                tool_call_id: String::new(),
                content: "Execution denied by security policy.".to_string(),
                is_error: true,
            });
        }

        // Handle background execution
        let background = params["background"].as_bool().unwrap_or(false);
        let yield_ms = params["yieldMs"].as_u64();

        if background {
            return spawn_background(command, workdir).await;
        }

        info!("exec: {} (cwd: {})", command, workdir);

        let shell = if cfg!(windows) { "powershell" } else { "sh" };
        let shell_arg = if cfg!(windows) { "-Command" } else { "-c" };

        // If yieldMs is set, race between completion and timeout
        let effective_timeout = if let Some(ms) = yield_ms {
            std::cmp::min(timeout_secs * 1000, ms) as u64
        } else {
            timeout_secs * 1000
        };

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(effective_timeout),
            Command::new(shell)
                .arg(shell_arg)
                .arg(command)
                .current_dir(workdir)
                .output()
        ).await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let code = output.status.code().unwrap_or(-1);

                let content = if stderr.is_empty() {
                    format!("{}\n(exit code: {})", stdout.trim(), code)
                } else {
                    format!("{}\nSTDERR: {}\n(exit code: {})", stdout.trim(), stderr.trim(), code)
                };

                // Truncate to 4KB
                let content = if content.len() > 4096 {
                    format!("{}...\n(truncated, {} total chars)", &content[..4000], content.len())
                } else {
                    content
                };

                Ok(ToolResult {
                    tool_call_id: String::new(),
                    content,
                    is_error: !output.status.success(),
                })
            }
            Ok(Err(e)) => Ok(ToolResult {
                tool_call_id: String::new(),
                content: format!("Failed to execute: {}", e),
                is_error: true,
            }),
            Err(_) => {
                // Timed out — if yieldMs was set, background it
                if yield_ms.is_some() {
                    spawn_background(command, workdir).await
                } else {
                    Ok(ToolResult {
                        tool_call_id: String::new(),
                        content: format!("Command timed out after {}s", timeout_secs),
                        is_error: true,
                    })
                }
            }
        }
    }
}

/// Spawn a command in the background and return session_id
async fn spawn_background(command: &str, workdir: &str) -> anyhow::Result<ToolResult> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let cmd = command.to_string();
    let wd = workdir.to_string();
    let sid = session_id.clone();

    tokio::spawn(async move {
        let shell = if cfg!(windows) { "powershell" } else { "sh" };
        let shell_arg = if cfg!(windows) { "-Command" } else { "-c" };

        let result = Command::new(shell)
            .arg(shell_arg)
            .arg(&cmd)
            .current_dir(&wd)
            .output()
            .await;

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let code = output.status.code().unwrap_or(-1);
                add_background_session(&sid, stdout, stderr, Some(code), true);
            }
            Err(e) => {
                add_background_session(&sid, String::new(), e.to_string(), None, true);
            }
        }
    });

    // Register as running
    add_background_session(&session_id, String::new(), String::new(), None, false);

    Ok(ToolResult {
        tool_call_id: String::new(),
        content: serde_json::json!({
            "status": "running",
            "sessionId": session_id
        }).to_string(),
        is_error: false,
    })
}
