use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;
use std::path::Path;

fn ok(content: String) -> ToolResult {
    ToolResult { tool_call_id: String::new(), content, is_error: false }
}

fn err(content: String) -> ToolResult {
    ToolResult { tool_call_id: String::new(), content, is_error: true }
}

pub struct ImageTool;

#[async_trait]
impl Tool for ImageTool {
    fn name(&self) -> &str { "image" }
    fn description(&self) -> &str { "Analyze images with a vision model." }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "image": { "type": "string", "description": "Image path or URL" },
                "prompt": { "type": "string", "description": "Analysis prompt" }
            }
        })
    }
    async fn execute(&self, params: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let image_ref = params["image"].as_str()
            .or_else(|| params["path"].as_str());
        let prompt = params["prompt"].as_str().unwrap_or("Describe this image.");

        let image_ref = match image_ref {
            Some(r) => r,
            None => return Ok(err("Missing 'image' parameter (path or URL).".into())),
        };

        // URL-based
        if image_ref.starts_with("http://") || image_ref.starts_with("https://") {
            return Ok(ok(format!(
                "Image URL: {}\nPrompt: {}\n\nURL-based image analysis requires routing to a vision-capable model. \
                 The gateway should forward this to a model that supports image inputs (GPT-4V, Claude 3, etc.).",
                image_ref, prompt
            )));
        }

        // Local file
        let path = if Path::new(image_ref).is_absolute() {
            image_ref.to_string()
        } else {
            format!("{}/{}", ctx.workspace_dir, image_ref)
        };

        if !Path::new(&path).exists() {
            return Ok(err(format!("Image file not found: {}", path)));
        }

        match tokio::fs::read(&path).await {
            Ok(bytes) => {
                let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
                let size_kb = bytes.len() / 1024;
                let ext = Path::new(&path).extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("png");
                let mime = match ext {
                    "jpg" | "jpeg" => "image/jpeg",
                    "png" => "image/png",
                    "gif" => "image/gif",
                    "webp" => "image/webp",
                    _ => "application/octet-stream",
                };

                Ok(ok(format!(
                    "Image loaded: {} ({} KB, {})\nPrompt: {}\nBase64 length: {} chars\n\n\
                     To analyze: route this to a vision model with the base64 data as:\n\
                     data:{};base64,{}...",
                    path, size_kb, mime, prompt, b64.len(), mime, &b64[..b64.len().min(100)]
                )))
            }
            Err(e) => Ok(err(format!("Failed to read image: {}", e))),
        }
    }
}
