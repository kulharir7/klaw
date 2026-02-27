use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;
use std::path::PathBuf;

fn klaw_home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".klaw")
}

fn ok(content: String) -> ToolResult {
    ToolResult { tool_call_id: String::new(), content, is_error: false }
}

fn err(content: String) -> ToolResult {
    ToolResult { tool_call_id: String::new(), content, is_error: true }
}

pub struct GatewayTool;

#[async_trait]
impl Tool for GatewayTool {
    fn name(&self) -> &str { "gateway" }
    fn description(&self) -> &str { "Control the gateway daemon. Actions: restart, config.get, config.apply, config.patch." }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["restart", "config.get", "config.apply", "config.patch"] },
                "config": { "type": "object", "description": "Config JSON (for config.apply and config.patch)" }
            },
            "required": ["action"]
        })
    }
    async fn execute(&self, params: Value, _ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let action = match params["action"].as_str() {
            Some(a) => a,
            None => return Ok(err("Missing 'action' parameter.".into())),
        };

        let home = klaw_home();

        match action {
            "restart" => {
                std::fs::create_dir_all(&home).ok();
                std::fs::write(home.join("gateway_restart"), "restart").ok();
                Ok(ok("Gateway restart requested.".into()))
            }
            "config.get" => {
                let config_path = home.join("klaw.json");
                if !config_path.exists() {
                    return Ok(ok("No config file found at ~/.klaw/klaw.json".into()));
                }
                match tokio::fs::read_to_string(&config_path).await {
                    Ok(content) => Ok(ok(content)),
                    Err(e) => Ok(err(format!("Failed to read config: {}", e))),
                }
            }
            "config.apply" => {
                let config = &params["config"];
                if config.is_null() {
                    return Ok(err("Missing 'config' parameter.".into()));
                }
                std::fs::create_dir_all(&home).ok();
                match std::fs::write(home.join("klaw.json"), serde_json::to_string_pretty(config).unwrap()) {
                    Ok(_) => Ok(ok("Config applied.".into())),
                    Err(e) => Ok(err(format!("Failed to write config: {}", e))),
                }
            }
            "config.patch" => {
                let patch = &params["config"];
                if patch.is_null() || !patch.is_object() {
                    return Ok(err("Missing 'config' object for patching.".into()));
                }
                let config_path = home.join("klaw.json");
                let mut existing: Value = if config_path.exists() {
                    let content = tokio::fs::read_to_string(&config_path).await.unwrap_or_default();
                    serde_json::from_str(&content).unwrap_or(Value::Object(Default::default()))
                } else {
                    Value::Object(Default::default())
                };

                // Shallow merge
                if let (Some(existing_obj), Some(patch_obj)) = (existing.as_object_mut(), patch.as_object()) {
                    for (k, v) in patch_obj {
                        existing_obj.insert(k.clone(), v.clone());
                    }
                }

                std::fs::create_dir_all(&home).ok();
                match std::fs::write(&config_path, serde_json::to_string_pretty(&existing).unwrap()) {
                    Ok(_) => Ok(ok("Config patched.".into())),
                    Err(e) => Ok(err(format!("Failed to write config: {}", e))),
                }
            }
            _ => Ok(err(format!("Unknown action: '{}'", action))),
        }
    }
}
