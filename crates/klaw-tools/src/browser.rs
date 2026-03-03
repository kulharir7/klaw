use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;

fn ok(content: String) -> ToolResult {
    ToolResult { tool_call_id: String::new(), content, is_error: false }
}

fn err(content: String) -> ToolResult {
    ToolResult { tool_call_id: String::new(), content, is_error: true }
}

pub struct BrowserTool;

impl BrowserTool {
    /// Check if Chrome DevTools Protocol is available
    async fn is_cdp_available() -> bool {
        tokio::net::TcpStream::connect("127.0.0.1:9222").await.is_ok()
    }

    /// Send a CDP command and get response
    async fn cdp_command(method: &str, params: Value) -> anyhow::Result<Value> {
        let client = reqwest::Client::new();
        let body = serde_json::json!({
            "id": 1,
            "method": method,
            "params": params
        });

        let resp = client
            .post("http://127.0.0.1:9222/json")
            .json(&body)
            .send()
            .await?;

        let result: Value = resp.json().await?;
        Ok(result)
    }

    /// Get list of open tabs
    async fn get_tabs() -> anyhow::Result<Vec<Value>> {
        let client = reqwest::Client::new();
        let resp = client.get("http://127.0.0.1:9222/json").send().await?;
        let tabs: Vec<Value> = resp.json().await?;
        Ok(tabs)
    }
}

#[async_trait]
impl Tool for BrowserTool {
    fn name(&self) -> &str { "browser" }
    fn description(&self) -> &str { "Control web browser via Chrome DevTools Protocol. Actions: status, open, close, navigate, snapshot, screenshot, tabs, act, click, type, evaluate." }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["status", "open", "close", "navigate", "snapshot", "screenshot", "tabs", "act", "click", "type", "evaluate"] },
                "url": { "type": "string", "description": "URL to navigate to or open" },
                "targetId": { "type": "string", "description": "Target/tab ID for operations" },
                "selector": { "type": "string", "description": "CSS selector for actions" },
                "text": { "type": "string", "description": "Text to type" },
                "script": { "type": "string", "description": "JavaScript to evaluate" },
                "fullPage": { "type": "boolean", "description": "Full page screenshot" }
            },
            "required": ["action"]
        })
    }
    async fn execute(&self, params: Value, _ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let action = params["action"].as_str().unwrap_or("status").to_string();

        // Check CDP availability first
        if Self::is_cdp_available().await {
            return self.handle_cdp_action(&action, params).await;
        }

        // CDP not available - provide helpful message
        self.handle_offline_action(&action, params).await
    }
}

impl BrowserTool {
    async fn handle_cdp_action(&self, action: &str, params: Value) -> anyhow::Result<ToolResult> {
        match action {
            "status" => {
                let tabs = Self::get_tabs().await.unwrap_or_default();
                Ok(ok(format!(
                    "🌐 Browser connected!\n\
                     • CDP: http://127.0.0.1:9222\n\
                     • Open tabs: {}\n\
                     Actions available: open, close, navigate, snapshot, screenshot, tabs, act, click, type, evaluate",
                    tabs.len()
                )))
            }

            "tabs" => {
                match Self::get_tabs().await {
                    Ok(tabs) => {
                        if tabs.is_empty() {
                            Ok(ok("No open tabs.".into()))
                        } else {
                            let tab_list: Vec<String> = tabs.iter().enumerate().map(|(i, t)| {
                                let title = t["title"].as_str().unwrap_or("Untitled");
                                let url = t["url"].as_str().unwrap_or("about:blank");
                                let id = t["id"].as_str().unwrap_or("?");
                                format!("{}. {} - {} [{}]", i + 1, title, url, id)
                            }).collect();
                            Ok(ok(format!("Open tabs ({}):\n{}", tabs.len(), tab_list.join("\n"))))
                        }
                    }
                    Err(e) => Ok(err(format!("Failed to get tabs: {}", e))),
                }
            }

            "open" => {
                let url = params["url"].as_str().unwrap_or("about:blank");
                let client = reqwest::Client::new();
                let body = serde_json::json!({
                    "url": url
                });
                match client.post("http://127.0.0.1:9222/json/new").json(&body).send().await {
                    Ok(resp) => {
                        let tab: Value = resp.json().await.unwrap_or(Value::Null);
                        Ok(ok(format!("Opened new tab: {}\nTab ID: {}", url, tab["id"].as_str().unwrap_or("?"))))
                    }
                    Err(e) => Ok(err(format!("Failed to open tab: {}", e))),
                }
            }

            "navigate" => {
                let url = params["url"].as_str().unwrap_or("about:blank");
                match Self::cdp_command("Page.navigate", serde_json::json!({"url": url})).await {
                    Ok(_) => Ok(ok(format!("Navigated to: {}", url))),
                    Err(e) => Ok(err(format!("Navigate failed: {}", e))),
                }
            }

            "screenshot" => {
                let full_page = params["fullPage"].as_bool().unwrap_or(false);
                let params = if full_page {
                    serde_json::json!({"format": "png", "captureBeyondViewport": true})
                } else {
                    serde_json::json!({"format": "png"})
                };
                match Self::cdp_command("Page.captureScreenshot", params).await {
                    Ok(result) => {
                        if let Some(data) = result["data"].as_str() {
                            let output_dir = dirs::home_dir()
                                .unwrap_or_else(|| std::path::PathBuf::from("."))
                                .join(".klaw")
                                .join("screenshots");
                            std::fs::create_dir_all(&output_dir).ok();
                            let filename = output_dir.join(format!("screenshot_{}.png", chrono::Local::now().format("%Y%m%d_%H%M%S")));
                            let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data).unwrap_or_default();
                            std::fs::write(&filename, &bytes).ok();
                            Ok(ok(format!("Screenshot saved: {}\nSize: {} KB", filename.display(), bytes.len() / 1024)))
                        } else {
                            Ok(err("No screenshot data in response".into()))
                        }
                    }
                    Err(e) => Ok(err(format!("Screenshot failed: {}", e))),
                }
            }

            "evaluate" => {
                let script = match params["script"].as_str() {
                    Some(s) => s,
                    None => return Ok(err("Missing 'script' parameter".into())),
                };
                match Self::cdp_command("Runtime.evaluate", serde_json::json!({"expression": script, "returnByValue": true})).await {
                    Ok(result) => {
                        if let Some(error) = result["exceptionDetails"].as_object() {
                            Ok(err(format!("Script error: {:?}", error)))
                        } else if let Some(value) = result["result"]["value"].as_str() {
                            Ok(ok(format!("Result: {}", value)))
                        } else {
                            Ok(ok(serde_json::to_string_pretty(&result["result"]).unwrap_or_else(|_| "OK".to_string())))
                        }
                    }
                    Err(e) => Ok(err(format!("Evaluate failed: {}", e))),
                }
            }

            "snapshot" => {
                // Get DOM snapshot
                match Self::cdp_command("DOMSnapshot.captureSnapshot", serde_json::json!({})).await {
                    Ok(result) => Ok(ok(format!("DOM Snapshot:\n{}", serde_json::to_string_pretty(&result).unwrap_or_else(|_| "snapshot captured".to_string())))),
                    Err(e) => Ok(err(format!("Snapshot failed: {}", e))),
                }
            }

            "click" | "type" | "act" => {
                let selector = match params["selector"].as_str() {
                    Some(s) => s,
                    None => return Ok(err("Missing 'selector' parameter".into())),
                };
                
                let script = if action == "click" || action == "act" {
                    format!("document.querySelector('{}').click()", selector)
                } else {
                    let text = params["text"].as_str().unwrap_or("");
                    format!("document.querySelector('{}').value = '{}'", selector, text)
                };

                match Self::cdp_command("Runtime.evaluate", serde_json::json!({"expression": script})).await {
                    Ok(_) => Ok(ok(format!("Action '{}' performed on '{}'", action, selector))),
                    Err(e) => Ok(err(format!("Action failed: {}", e))),
                }
            }

            "close" => {
                let target_id = match params["targetId"].as_str() {
                    Some(id) => id,
                    None => return Ok(err("Missing 'targetId' parameter. Use 'tabs' to list tab IDs.".into())),
                };
                let client = reqwest::Client::new();
                match client.get(&format!("http://127.0.0.1:9222/json/close/{}", target_id)).send().await {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            Ok(ok(format!("Tab {} closed.", target_id)))
                        } else {
                            Ok(err(format!("Failed to close tab: {}", resp.status())))
                        }
                    }
                    Err(e) => Ok(err(format!("Close failed: {}", e))),
                }
            }

            _ => Ok(err(format!("Unknown action: '{}'. Available: status, tabs, open, close, navigate, snapshot, screenshot, click, type, evaluate", action))),
        }
    }

    async fn handle_offline_action(&self, action: &str, params: Value) -> anyhow::Result<ToolResult> {
        match action {
            "status" => Ok(ok(
                "🌐 Browser not connected.\n\
                 Start Chrome with: chrome --remote-debugging-port=9222\n\
                 Or on Mac: /Applications/Google\\ Chrome.app/Contents/MacOS/Google\\ Chrome --remote-debugging-port=9222\n\
                 Or on Windows: chrome.exe --remote-debugging-port=9222".into()
            )),

            "open" => {
                let url = params["url"].as_str().unwrap_or("about:blank");
                // Try to open URL in default browser as fallback
                let _ = opener::open(url);
                Ok(ok(format!(
                    "Opened URL in default browser: {}\n\
                     Note: Browser automation requires Chrome with --remote-debugging-port=9222",
                    url
                )))
            }

            _ => Ok(ok(format!(
                "⚙️ Browser action '{}' requires Chrome DevTools Protocol connection.\n\
                 Start Chrome with: chrome --remote-debugging-port=9222\n\
                 Then retry this action.",
                action
            ))),
        }
    }
}
