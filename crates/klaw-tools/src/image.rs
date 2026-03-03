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
    fn description(&self) -> &str { "Analyze images with a vision model (OpenAI GPT-4V, Claude 3, etc.). Supports local files and URLs." }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "image": { "type": "string", "description": "Image path or URL" },
                "prompt": { "type": "string", "description": "Analysis prompt (default: 'Describe this image.')" },
                "model": { "type": "string", "description": "Vision model to use (default: from config)" },
                "detail": { "type": "string", "enum": ["low", "high", "auto"], "description": "Detail level for OpenAI models" }
            },
            "required": ["image"]
        })
    }
    async fn execute(&self, params: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let image_ref = params["image"].as_str()
            .or_else(|| params["path"].as_str());
        let prompt = params["prompt"].as_str().unwrap_or("Describe this image in detail.");
        let model = params["model"].as_str();
        let detail = params["detail"].as_str().unwrap_or("auto");

        let image_ref = match image_ref {
            Some(r) => r,
            None => return Ok(err("Missing 'image' parameter (path or URL).".into())),
        };

        // Get API key from environment or config
        let api_key = std::env::var("OPENAI_API_KEY")
            .or_else(|_| std::env::var("ANTHROPIC_API_KEY"))
            .ok();

        // URL-based image
        if image_ref.starts_with("http://") || image_ref.starts_with("https://") {
            // For URLs, we can either download or pass directly to vision API
            let image_url = serde_json::json!({
                "type": "image_url",
                "image_url": { "url": image_ref, "detail": detail }
            });

            return if api_key.is_some() {
                // Attempt to call vision API
                match call_vision_api(&image_url, prompt, model, &api_key.unwrap()).await {
                    Ok(result) => Ok(ok(result)),
                    Err(e) => Ok(err(format!("Vision API error: {}", e))),
                }
            } else {
                Ok(ok(format!(
                    "Image URL: {}\nPrompt: {}\n\nSet OPENAI_API_KEY or ANTHROPIC_API_KEY environment variable to enable vision analysis.",
                    image_ref, prompt
                )))
            };
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
                let size_kb = bytes.len() / 1024;
                
                // Check size limit (most APIs have ~20MB limit)
                if size_kb > 20 * 1024 {
                    return Ok(err(format!("Image too large: {} KB. Maximum is 20 MB.", size_kb)));
                }

                let ext = Path::new(&path).extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("png")
                    .to_lowercase();
                
                let mime = match ext.as_str() {
                    "jpg" | "jpeg" => "image/jpeg",
                    "png" => "image/png",
                    "gif" => "image/gif",
                    "webp" => "image/webp",
                    _ => "application/octet-stream",
                };

                let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
                let data_url = format!("data:{};base64,{}", mime, b64);

                let image_content = serde_json::json!({
                    "type": "image_url",
                    "image_url": { "url": data_url, "detail": detail }
                });

                if let Some(key) = api_key {
                    match call_vision_api(&image_content, prompt, model, &key).await {
                        Ok(result) => Ok(ok(result)),
                        Err(e) => Ok(err(format!("Vision API error: {}", e))),
                    }
                } else {
                    // Return info about what would be sent
                    Ok(ok(format!(
                        "📷 Image loaded successfully!\n\
                         • Path: {}\n\
                         • Size: {} KB\n\
                         • Format: {}\n\
                         • Prompt: {}\n\
                         • Base64: {} characters\n\n\
                         ⚠️ Set OPENAI_API_KEY or ANTHROPIC_API_KEY to enable vision analysis.",
                        path, size_kb, mime, prompt, b64.len()
                    )))
                }
            }
            Err(e) => Ok(err(format!("Failed to read image: {}", e))),
        }
    }
}

/// Call vision API (OpenAI GPT-4V or compatible)
async fn call_vision_api(
    image_content: &Value,
    prompt: &str,
    model: Option<&str>,
    api_key: &str,
) -> anyhow::Result<String> {
    let model_name = model.unwrap_or("gpt-4o");
    let is_anthropic = api_key.starts_with("sk-ant");
    
    let client = reqwest::Client::new();
    
    if is_anthropic {
        // Anthropic Claude format
        let body = serde_json::json!({
            "model": model_name,
            "max_tokens": 4096,
            "messages": [{
                "role": "user",
                "content": [
                    { "type": "text", "text": prompt },
                    image_content.clone()
                ]
            }]
        });
        
        let resp = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;
        
        let result: Value = resp.json().await?;
        
        if let Some(content) = result["content"][0]["text"].as_str() {
            Ok(content.to_string())
        } else if let Some(error) = result["error"]["message"].as_str() {
            Err(anyhow::anyhow!("API error: {}", error))
        } else {
            Ok(serde_json::to_string_pretty(&result)?)
        }
    } else {
        // OpenAI format
        let body = serde_json::json!({
            "model": model_name,
            "messages": [{
                "role": "user",
                "content": [
                    { "type": "text", "text": prompt },
                    image_content.clone()
                ]
            }],
            "max_tokens": 4096
        });
        
        let resp = client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;
        
        let result: Value = resp.json().await?;
        
        if let Some(content) = result["choices"][0]["message"]["content"].as_str() {
            Ok(content.to_string())
        } else if let Some(error) = result["error"]["message"].as_str() {
            Err(anyhow::anyhow!("API error: {}", error))
        } else {
            Ok(serde_json::to_string_pretty(&result)?)
        }
    }
}
