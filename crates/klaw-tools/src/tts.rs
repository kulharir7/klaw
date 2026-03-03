use crate::{Tool, ToolContext};
use async_trait::async_trait;
use klaw_core::types::ToolResult;
use serde_json::Value;
use std::path::PathBuf;
use std::io::Write;

fn klaw_home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".klaw")
}

fn ok(content: String) -> ToolResult {
    ToolResult { tool_call_id: String::new(), content, is_error: false }
}

fn err(content: String) -> ToolResult {
    ToolResult { tool_call_id: String::new(), content, is_error: true }
}

pub struct TtsTool;

#[async_trait]
impl Tool for TtsTool {
    fn name(&self) -> &str { "tts" }
    fn description(&self) -> &str { "Convert text to speech using OpenAI TTS API. Returns audio file path or plays directly." }
    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "Text to convert to speech" },
                "voice": { "type": "string", "enum": ["alloy", "echo", "fable", "onyx", "nova", "shimmer"], "description": "Voice to use (default: alloy)" },
                "model": { "type": "string", "enum": ["tts-1", "tts-1-hd"], "description": "TTS model (default: tts-1)" },
                "output": { "type": "string", "description": "Output filename (default: auto-generated)" },
                "play": { "type": "boolean", "description": "Play audio after generation (default: false)" }
            },
            "required": ["text"]
        })
    }
    async fn execute(&self, params: Value, _ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        let text = match params["text"].as_str() {
            Some(t) => t,
            None => return Ok(err("Missing 'text' parameter.".into())),
        };

        let voice = params["voice"].as_str().unwrap_or("alloy");
        let model = params["model"].as_str().unwrap_or("tts-1");
        let output_name = params["output"].as_str();
        let should_play = params["play"].as_bool().unwrap_or(false);

        // Check for API key
        let api_key = match std::env::var("OPENAI_API_KEY") {
            Ok(k) => k,
            Err(_) => return Ok(ok(format!(
                "🔊 TTS requires OPENAI_API_KEY environment variable.\n\
                 Text length: {} characters\n\
                 Voice: {}\n\
                 Model: {}\n\n\
                 Supported voices: alloy, echo, fable, onyx, nova, shimmer\n\
                 Supported models: tts-1 (fast), tts-1-hd (high quality)",
                text.len(), voice, model
            ))),
        };

        // Generate output filename
        let output_dir = klaw_home().join("tts");
        std::fs::create_dir_all(&output_dir).ok();
        
        let filename = match output_name {
            Some(name) => {
                let name = if name.ends_with(".mp3") { name.to_string() } else { format!("{}.mp3", name) };
                output_dir.join(name)
            }
            None => {
                let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
                output_dir.join(format!("tts_{}.mp3", timestamp))
            }
        };

        // Call OpenAI TTS API
        let client = reqwest::Client::new();
        let body = serde_json::json!({
            "model": model,
            "input": text,
            "voice": voice
        });

        let response = client
            .post("https://api.openai.com/v1/audio/speech")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await;

        match response {
            Ok(resp) => {
                if !resp.status().is_success() {
                    let status = resp.status();
                    let error_text = resp.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                    return Ok(err(format!("OpenAI TTS API error ({}): {}", status, error_text)));
                }

                let audio_bytes = resp.bytes().await;
                match audio_bytes {
                    Ok(bytes) => {
                        // Save to file
                        let file_result = std::fs::File::create(&filename);
                        match file_result {
                            Ok(mut file) => {
                                if let Err(e) = file.write_all(&bytes) {
                                    return Ok(err(format!("Failed to write audio file: {}", e)));
                                }

                                let size_kb = bytes.len() / 1024;
                                let output_path = filename.to_string_lossy();
                                
                                // Optionally play the audio
                                if should_play {
                                    #[cfg(target_os = "macos")]
                                    let play_cmd = format!("afplay \"{}\"", output_path);
                                    #[cfg(target_os = "windows")]
                                    let play_cmd = format!("powershell -c (New-Object Media.SoundPlayer \"{}\").PlaySync()", output_path);
                                    #[cfg(target_os = "linux")]
                                    let play_cmd = format!("aplay \"{}\" 2>/dev/null || mpv \"{}\" 2>/dev/null || echo 'No audio player found'", output_path, output_path);
                                    
                                    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
                                    {
                                        let _ = std::process::Command::new("sh")
                                            .arg("-c")
                                            .arg(&play_cmd)
                                            .spawn();
                                    }
                                }

                                Ok(ok(format!(
                                    "🔊 TTS audio generated!\n\
                                     • File: {}\n\
                                     • Size: {} KB\n\
                                     • Voice: {}\n\
                                     • Model: {}\n\
                                     • Text length: {} chars{}",
                                    output_path,
                                    size_kb,
                                    voice,
                                    model,
                                    text.len(),
                                    if should_play { "\n• Playing audio..." } else { "" }
                                )))
                            }
                            Err(e) => Ok(err(format!("Failed to create audio file: {}", e))),
                        }
                    }
                    Err(e) => Ok(err(format!("Failed to read audio response: {}", e))),
                }
            }
            Err(e) => Ok(err(format!("Failed to call OpenAI TTS API: {}", e))),
        }
    }
}
