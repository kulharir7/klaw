use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;

fn ok(content: String) -> ToolResult {
    ToolResult { tool_call_id: String::new(), content, is_error: false }
}

pub struct BrowserTool;

#[async_trait]
impl Tool for BrowserTool {
    fn name(&self) -> &str { "browser" }
    fn description(&self) -> &str { "Control web browser for automation." }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["status", "snapshot", "screenshot", "navigate", "act", "open", "close"] },
                "url": { "type": "string" },
                "ref": { "type": "string" }
            },
            "required": ["action"]
        })
    }
    async fn execute(&self, params: Value, _ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let action = params["action"].as_str().unwrap_or("status");

        match action {
            "status" => {
                // Try to connect to Chrome DevTools
                let connected = std::net::TcpStream::connect("127.0.0.1:9222").is_ok();
                if connected {
                    Ok(ok("Browser status: Chrome DevTools Protocol connected on port 9222.".into()))
                } else {
                    Ok(ok("Browser status: Not connected.\nStart Chrome with: chrome --remote-debugging-port=9222".into()))
                }
            }
            "snapshot" => {
                Ok(ok("Browser snapshot requires a CDP or Playwright connection.\n\
                       Start Chrome with: chrome --remote-debugging-port=9222\n\
                       Then use 'status' to verify connection.".into()))
            }
            "navigate" => {
                let url = params["url"].as_str().unwrap_or("");
                Ok(ok(format!(
                    "Browser navigation requires CDP connection.\n\
                     Target URL: {}\n\
                     Start Chrome with: chrome --remote-debugging-port=9222",
                    if url.is_empty() { "(none specified)" } else { url }
                )))
            }
            _ => {
                Ok(ok(format!(
                    "Browser action '{}' requires an active CDP connection.\n\
                     Start Chrome with: chrome --remote-debugging-port=9222\n\
                     Full browser automation will be available once CDP integration is complete.",
                    action
                )))
            }
        }
    }
}
