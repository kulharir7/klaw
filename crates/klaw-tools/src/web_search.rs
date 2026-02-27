use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;
use tracing::info;

pub struct WebSearchTool {
    api_key: Option<String>,
}

impl WebSearchTool {
    pub fn new(api_key: Option<String>) -> Self {
        Self { api_key }
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str { "web_search" }

    fn description(&self) -> &str {
        "Search the web using Brave Search API. Returns titles, URLs, and snippets."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query string"
                },
                "count": {
                    "type": "number",
                    "description": "Number of results (1-10, default 5)"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let query = params["query"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'query' parameter"))?;
        let count = params["count"].as_u64().unwrap_or(5).min(10);

        let env_key = std::env::var("BRAVE_API_KEY").ok();
        let api_key = self.api_key.as_deref()
            .or(env_key.as_deref())
            .ok_or_else(|| anyhow::anyhow!("No Brave Search API key configured"))?
            .to_string();

        info!("web_search: {} (count: {})", query, count);

        let client = reqwest::Client::new();
        let response = client
            .get("https://api.search.brave.com/res/v1/web/search")
            .header("X-Subscription-Token", &api_key)
            .query(&[("q", query), ("count", &count.to_string())])
            .send()
            .await?;

        if !response.status().is_success() {
            return Ok(ToolResult {
                tool_call_id: String::new(),
                content: format!("Search API error: {}", response.status()),
                is_error: true,
            });
        }

        let data: Value = response.json().await?;
        let mut results = Vec::new();

        if let Some(web) = data["web"]["results"].as_array() {
            for (i, r) in web.iter().enumerate().take(count as usize) {
                let title = r["title"].as_str().unwrap_or("No title");
                let url = r["url"].as_str().unwrap_or("");
                let desc = r["description"].as_str().unwrap_or("No description");
                results.push(format!("{}. {}\n   {}\n   {}", i + 1, title, url, desc));
            }
        }

        let content = if results.is_empty() {
            "No results found.".to_string()
        } else {
            results.join("\n\n")
        };

        Ok(ToolResult {
            tool_call_id: String::new(),
            content,
            is_error: false,
        })
    }
}
