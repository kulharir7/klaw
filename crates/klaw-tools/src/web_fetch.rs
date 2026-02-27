use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;
use tracing::info;

pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str { "web_fetch" }

    fn description(&self) -> &str {
        "Fetch and extract readable content from a URL (HTML to text). Use for lightweight page access."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "HTTP or HTTPS URL to fetch"
                },
                "max_chars": {
                    "type": "number",
                    "description": "Maximum characters to return (default 50000)"
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let url = params["url"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'url' parameter"))?;
        let max_chars = params["max_chars"].as_u64().unwrap_or(50000) as usize;

        info!("web_fetch: {}", url);

        // Basic SSRF protection
        if url.starts_with("file://") || url.contains("localhost") || url.contains("127.0.0.1") {
            return Ok(ToolResult {
                tool_call_id: String::new(),
                content: "Blocked: local/internal URLs not allowed".to_string(),
                is_error: true,
            });
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let response = client.get(url)
            .header("User-Agent", "Klaw/0.1 (AI Gateway)")
            .send()
            .await?;

        if !response.status().is_success() {
            return Ok(ToolResult {
                tool_call_id: String::new(),
                content: format!("HTTP error: {}", response.status()),
                is_error: true,
            });
        }

        let body = response.text().await?;

        // Simple HTML to text extraction (strip tags)
        let text = strip_html(&body);

        // Truncate
        let text = if text.len() > max_chars {
            format!("{}...\n(truncated at {} chars, total {} chars)", &text[..max_chars], max_chars, text.len())
        } else {
            text
        };

        Ok(ToolResult {
            tool_call_id: String::new(),
            content: text,
            is_error: false,
        })
    }
}

/// Simple HTML tag stripper (basic — no full parser)
fn strip_html(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_script = false;
    let mut last_was_space = false;

    let lower = html.to_lowercase();
    let chars: Vec<char> = html.chars().collect();
    let lower_chars: Vec<char> = lower.chars().collect();

    let mut i = 0;
    while i < chars.len() {
        if !in_tag && i + 7 < lower_chars.len() {
            let ahead: String = lower_chars[i..i+7].iter().collect();
            if ahead == "<script" || ahead == "<style " || ahead == "<style>" {
                in_script = true;
                in_tag = true;
                i += 1;
                continue;
            }
        }

        if in_script && i + 8 < lower_chars.len() {
            let ahead: String = lower_chars[i..i+8].iter().collect();
            if ahead == "</script" || ahead == "</style>" {
                in_script = false;
            }
            let ahead9: String = lower_chars[i..std::cmp::min(i+9, lower_chars.len())].iter().collect();
            if ahead9 == "</script>" || ahead9 == "</style>>" {
                in_script = false;
            }
        }

        if chars[i] == '<' {
            in_tag = true;
            // Add newline for block elements
            if i + 3 < lower_chars.len() {
                let tag: String = lower_chars[i+1..std::cmp::min(i+4, lower_chars.len())].iter().collect();
                if tag.starts_with("br") || tag.starts_with("/p") || tag.starts_with("/d") || tag.starts_with("/h") || tag.starts_with("/li") || tag.starts_with("/tr") {
                    if !last_was_space {
                        result.push('\n');
                        last_was_space = true;
                    }
                }
            }
        } else if chars[i] == '>' {
            in_tag = false;
        } else if !in_tag && !in_script {
            let c = chars[i];
            if c.is_whitespace() {
                if !last_was_space {
                    result.push(' ');
                    last_was_space = true;
                }
            } else {
                result.push(c);
                last_was_space = false;
            }
        }
        i += 1;
    }

    // Decode basic HTML entities
    result
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
}
