pub mod registry;
pub mod exec;
pub mod read;
pub mod write;
pub mod edit;
pub mod web_search;
pub mod web_fetch;
pub mod process;
pub mod apply_patch;
pub mod memory_search;
pub mod memory_get;
pub mod image;
pub mod tts;
pub mod message;
pub mod cron;
pub mod gateway_tool;
pub mod sessions;
pub mod browser;
pub mod canvas_tool;
pub mod nodes;

use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;

/// Context passed to every tool execution
#[derive(Debug, Clone)]
pub struct ToolContext {
    pub workspace_dir: String,
    pub session_key: String,
    pub agent_id: String,
}

/// Every tool implements this trait
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Value;
    async fn execute(&self, params: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult>;
}

/// Create a default tool registry with all core tools
pub fn create_default_registry(brave_api_key: Option<String>) -> registry::ToolRegistry {
    let mut reg = registry::ToolRegistry::new();
    reg.register(std::sync::Arc::new(exec::ExecTool));
    reg.register(std::sync::Arc::new(read::ReadTool));
    reg.register(std::sync::Arc::new(write::WriteTool));
    reg.register(std::sync::Arc::new(edit::EditTool));
    reg.register(std::sync::Arc::new(web_search::WebSearchTool::new(brave_api_key)));
    reg.register(std::sync::Arc::new(web_fetch::WebFetchTool));
    reg.register(std::sync::Arc::new(process::ProcessTool));
    reg.register(std::sync::Arc::new(apply_patch::ApplyPatchTool));
    reg.register(std::sync::Arc::new(memory_search::MemorySearchTool));
    reg.register(std::sync::Arc::new(memory_get::MemoryGetTool));
    reg.register(std::sync::Arc::new(image::ImageTool));
    reg.register(std::sync::Arc::new(tts::TtsTool));
    reg.register(std::sync::Arc::new(message::MessageTool));
    reg.register(std::sync::Arc::new(cron::CronTool));
    reg.register(std::sync::Arc::new(gateway_tool::GatewayTool));
    reg.register(std::sync::Arc::new(sessions::SessionsList));
    reg.register(std::sync::Arc::new(sessions::SessionsHistory));
    reg.register(std::sync::Arc::new(sessions::SessionsSend));
    reg.register(std::sync::Arc::new(sessions::SessionsSpawn));
    reg.register(std::sync::Arc::new(sessions::SessionStatus));
    reg.register(std::sync::Arc::new(sessions::AgentsList));
    reg.register(std::sync::Arc::new(browser::BrowserTool));
    reg.register(std::sync::Arc::new(canvas_tool::CanvasTool));
    reg.register(std::sync::Arc::new(nodes::NodesTool));
    reg
}
