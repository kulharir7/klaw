use crate::{Tool, ToolContext};
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
        "Execute shell commands. Use for running programs, scripts, git, etc. Returns stdout/stderr and exit code."
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

        info!("exec: {} (cwd: {})", command, workdir);

        let shell = if cfg!(windows) { "powershell" } else { "sh" };
        let shell_arg = if cfg!(windows) { "-Command" } else { "-c" };

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
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
            Err(_) => Ok(ToolResult {
                tool_call_id: String::new(),
                content: format!("Command timed out after {}s", timeout_secs),
                is_error: true,
            }),
        }
    }
}
