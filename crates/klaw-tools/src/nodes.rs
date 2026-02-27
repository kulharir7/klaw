use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;
use std::path::PathBuf;

fn klaw_home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".klaw")
}

fn pairing_file() -> PathBuf {
    klaw_home().join("pairing.json")
}

fn ok(content: String) -> ToolResult {
    ToolResult { tool_call_id: String::new(), content, is_error: false }
}

fn err(content: String) -> ToolResult {
    ToolResult { tool_call_id: String::new(), content, is_error: true }
}

fn read_devices() -> Vec<Value> {
    let path = pairing_file();
    if !path.exists() { return vec![]; }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_devices(devices: &[Value]) -> Result<(), String> {
    let path = pairing_file();
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&path, serde_json::to_string_pretty(devices).unwrap()).map_err(|e| e.to_string())
}

pub struct NodesTool;

#[async_trait]
impl Tool for NodesTool {
    fn name(&self) -> &str { "nodes" }
    fn description(&self) -> &str { "Discover and control paired nodes." }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["status", "describe", "pending", "approve", "reject", "notify", "run", "camera_snap", "screen_record", "location_get"] },
                "node": { "type": "string", "description": "Node/device ID" },
                "title": { "type": "string" },
                "body": { "type": "string" }
            },
            "required": ["action"]
        })
    }
    async fn execute(&self, params: Value, _ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let action = params["action"].as_str().unwrap_or("status");
        let node_id = params["node"].as_str().unwrap_or("");

        match action {
            "status" => {
                let devices = read_devices();
                if devices.is_empty() {
                    Ok(ok("No paired nodes found. Pairing registry is empty.".into()))
                } else {
                    Ok(ok(format!("Paired nodes ({}):\n{}", devices.len(), serde_json::to_string_pretty(&devices).unwrap())))
                }
            }
            "describe" => {
                if node_id.is_empty() {
                    return Ok(err("Missing 'node' parameter.".into()));
                }
                let devices = read_devices();
                let device = devices.iter().find(|d| d["id"].as_str() == Some(node_id));
                match device {
                    Some(d) => Ok(ok(serde_json::to_string_pretty(d).unwrap())),
                    None => Ok(err(format!("Node '{}' not found.", node_id))),
                }
            }
            "pending" => {
                let devices = read_devices();
                let pending: Vec<&Value> = devices.iter().filter(|d| d["status"].as_str() == Some("pending")).collect();
                if pending.is_empty() {
                    Ok(ok("No pending devices.".into()))
                } else {
                    Ok(ok(format!("Pending devices ({}):\n{}", pending.len(), serde_json::to_string_pretty(&pending).unwrap())))
                }
            }
            "approve" | "reject" => {
                if node_id.is_empty() {
                    return Ok(err("Missing 'node' parameter.".into()));
                }
                let mut devices = read_devices();
                let new_status = if action == "approve" { "approved" } else { "rejected" };
                let mut found = false;
                for d in &mut devices {
                    if d["id"].as_str() == Some(node_id) {
                        d["status"] = Value::String(new_status.into());
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Ok(err(format!("Node '{}' not found.", node_id)));
                }
                if let Err(e) = write_devices(&devices) {
                    return Ok(err(format!("Failed to save: {}", e)));
                }
                Ok(ok(format!("Node '{}' {}.", node_id, new_status)))
            }
            "notify" | "run" | "camera_snap" | "camera_list" | "camera_clip" |
            "screen_record" | "location_get" | "invoke" => {
                let id_display = if node_id.is_empty() { "(unspecified)" } else { node_id };
                Ok(ok(format!(
                    "Requires active node connection. Node '{}' is not currently connected.\n\
                     Action '{}' will be available when the node is online and paired.",
                    id_display, action
                )))
            }
            _ => Ok(err(format!("Unknown action: '{}'", action))),
        }
    }
}
