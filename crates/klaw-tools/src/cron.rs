use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;
use chrono::Local;
use std::path::PathBuf;

fn klaw_home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".klaw")
}

fn cron_file() -> PathBuf {
    klaw_home().join("cron_jobs.json")
}

fn ok(content: String) -> ToolResult {
    ToolResult { tool_call_id: String::new(), content, is_error: false }
}

fn err(content: String) -> ToolResult {
    ToolResult { tool_call_id: String::new(), content, is_error: true }
}

fn read_jobs() -> Vec<Value> {
    let path = cron_file();
    if !path.exists() { return vec![]; }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_jobs(jobs: &[Value]) -> Result<(), String> {
    let path = cron_file();
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&path, serde_json::to_string_pretty(jobs).unwrap()).map_err(|e| e.to_string())
}

pub struct CronTool;

#[async_trait]
impl Tool for CronTool {
    fn name(&self) -> &str { "cron" }
    fn description(&self) -> &str { "Schedule recurring tasks. Actions: list, add, remove, status, run, wake." }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["list", "add", "remove", "status", "run", "wake"] },
                "schedule": { "type": "string", "description": "Cron expression" },
                "task": { "type": "string", "description": "Task description" },
                "id": { "type": "string", "description": "Job ID (for remove/run)" }
            },
            "required": ["action"]
        })
    }
    async fn execute(&self, params: Value, _ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let action = match params["action"].as_str() {
            Some(a) => a,
            None => return Ok(err("Missing 'action' parameter.".into())),
        };

        match action {
            "list" => {
                let jobs = read_jobs();
                if jobs.is_empty() {
                    Ok(ok("No cron jobs configured.".into()))
                } else {
                    Ok(ok(serde_json::to_string_pretty(&jobs).unwrap()))
                }
            }
            "add" => {
                let schedule = params["schedule"].as_str().unwrap_or("").to_string();
                let task = params["task"].as_str().unwrap_or("").to_string();
                if task.is_empty() {
                    return Ok(err("Missing 'task' parameter.".into()));
                }
                let id = uuid::Uuid::new_v4().to_string();
                let job = serde_json::json!({
                    "id": id,
                    "schedule": schedule,
                    "task": task,
                    "enabled": true,
                    "created_at": Local::now().to_rfc3339(),
                    "last_run": null
                });
                let mut jobs = read_jobs();
                jobs.push(job);
                if let Err(e) = write_jobs(&jobs) {
                    return Ok(err(format!("Failed to save: {}", e)));
                }
                Ok(ok(format!("Job added with id: {}", id)))
            }
            "remove" => {
                let id = match params["id"].as_str() {
                    Some(id) => id,
                    None => return Ok(err("Missing 'id' parameter.".into())),
                };
                let mut jobs = read_jobs();
                let before = jobs.len();
                jobs.retain(|j| j["id"].as_str() != Some(id));
                if jobs.len() == before {
                    return Ok(err(format!("Job '{}' not found.", id)));
                }
                if let Err(e) = write_jobs(&jobs) {
                    return Ok(err(format!("Failed to save: {}", e)));
                }
                Ok(ok(format!("Job '{}' removed.", id)))
            }
            "status" => {
                let jobs = read_jobs();
                let enabled = jobs.iter().filter(|j| j["enabled"].as_bool() == Some(true)).count();
                Ok(ok(format!("Total jobs: {}\nEnabled: {}\nDisabled: {}", jobs.len(), enabled, jobs.len() - enabled)))
            }
            "run" => {
                let id = match params["id"].as_str() {
                    Some(id) => id,
                    None => return Ok(err("Missing 'id' parameter.".into())),
                };
                let mut jobs = read_jobs();
                let mut found = false;
                for job in &mut jobs {
                    if job["id"].as_str() == Some(id) {
                        job["last_run"] = Value::String(Local::now().to_rfc3339());
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Ok(err(format!("Job '{}' not found.", id)));
                }
                if let Err(e) = write_jobs(&jobs) {
                    return Ok(err(format!("Failed to save: {}", e)));
                }
                Ok(ok(format!("Job '{}' marked as run.", id)))
            }
            "wake" => {
                Ok(ok("Wake signal sent. (Gateway integration pending)".into()))
            }
            _ => Ok(err(format!("Unknown action: '{}'", action))),
        }
    }
}
