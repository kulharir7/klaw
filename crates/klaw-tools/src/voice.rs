//! Voice Message Processing
//!
//! Transcribe voice messages using Whisper API or local Whisper.cpp

use klaw_core::types::ToolResult;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Voice transcription configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceConfig {
    /// Enable voice transcription
    pub enabled: bool,
    /// Whisper model to use
    pub model: VoiceModel,
    /// Language code (e.g., "en", "es")
    pub language: Option<String>,
    /// API key for OpenAI Whisper
    pub api_key: Option<String>,
    /// Local Whisper.cpp path
    pub whisper_cpp_path: Option<PathBuf>,
    /// Maximum audio file size in MB
    pub max_file_size_mb: u32,
    /// Supported audio formats
    pub supported_formats: Vec<String>,
}

/// Whisper model selection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum VoiceModel {
    /// Tiny model (fastest, lowest quality)
    Tiny,
    /// Base model (fast)
    Base,
    /// Small model (balanced)
    Small,
    /// Medium model (better quality)
    Medium,
    /// Large model (best quality)
    Large,
    /// Turbo model (fast + good quality)
    Turbo,
}

impl Default for VoiceModel {
    fn default() -> Self {
        Self::Small
    }
}

impl std::fmt::Display for VoiceModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VoiceModel::Tiny => write!(f, "tiny"),
            VoiceModel::Base => write!(f, "base"),
            VoiceModel::Small => write!(f, "small"),
            VoiceModel::Medium => write!(f, "medium"),
            VoiceModel::Large => write!(f, "large"),
            VoiceModel::Turbo => write!(f, "turbo"),
        }
    }
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            model: VoiceModel::Small,
            language: None,
            api_key: None,
            whisper_cpp_path: None,
            max_file_size_mb: 25,
            supported_formats: vec![
                "mp3".to_string(),
                "mp4".to_string(),
                "mpeg".to_string(),
                "mpga".to_string(),
                "m4a".to_string(),
                "wav".to_string(),
                "webm".to_string(),
            ],
        }
    }
}

/// Voice transcription result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    /// Transcribed text
    pub text: String,
    /// Language detected
    pub language: Option<String>,
    /// Duration in seconds
    pub duration_seconds: Option<f64>,
    /// Word count
    pub word_count: usize,
    /// Tokens used (for API calls)
    pub tokens_used: Option<u64>,
    /// Model used
    pub model: String,
}

/// Voice message processor
pub struct VoiceProcessor {
    config: VoiceConfig,
    http_client: Option<reqwest::Client>,
}

impl VoiceProcessor {
    /// Create new voice processor
    pub fn new(config: VoiceConfig) -> Self {
        Self {
            config,
            http_client: Some(reqwest::Client::new()),
        }
    }
    
    /// Create with default config
    pub fn default_config() -> Self {
        Self::new(VoiceConfig::default())
    }
    
    /// Transcribe audio file
    pub async fn transcribe(&self, audio_path: &Path) -> Result<TranscriptionResult, VoiceError> {
        // Check if file exists
        if !audio_path.exists() {
            return Err(VoiceError::FileNotFound(audio_path.to_path_buf()));
        }
        
        // Check file extension
        let ext = audio_path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        
        if !self.config.supported_formats.contains(&ext.to_lowercase()) {
            return Err(VoiceError::UnsupportedFormat(ext.to_string()));
        }
        
        // Check file size
        let metadata = std::fs::metadata(audio_path)
            .map_err(|e| VoiceError::IoError(e.to_string()))?;
        let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
        
        if size_mb > self.config.max_file_size_mb as f64 {
            return Err(VoiceError::FileTooLarge {
                size_mb,
                max_mb: self.config.max_file_size_mb as f64,
            });
        }
        
        // Try API first, fall back to local
        if let Some(api_key) = &self.config.api_key {
            self.transcribe_via_api(audio_path, api_key).await
        } else if let Some(whisper_path) = &self.config.whisper_cpp_path {
            self.transcribe_via_whisper_cpp(audio_path, whisper_path).await
        } else {
            // Try to use local whisper.cpp if available
            self.transcribe_fallback(audio_path).await
        }
    }
    
    /// Transcribe via OpenAI Whisper API
    async fn transcribe_via_api(&self, audio_path: &Path, api_key: &str) -> Result<TranscriptionResult, VoiceError> {
        let client = self.http_client.as_ref().ok_or(VoiceError::NoClient)?;
        
        // Read file
        let file_bytes = std::fs::read(audio_path)
            .map_err(|e| VoiceError::IoError(e.to_string()))?;
        
        // Prepare multipart form
        let form = reqwest::multipart::Form::new()
            .part("file", reqwest::multipart::Part::bytes(file_bytes)
                .file_name(audio_path.file_name().unwrap().to_string_lossy().to_string())
                .mime_str("audio/mpeg")
                .map_err(|e| VoiceError::RequestError(e.to_string()))?)
            .text("model", format!("whisper-{}", self.config.model));
        
        // Add language if specified
        let form = if let Some(lang) = &self.config.language {
            form.text("language", lang.clone())
        } else {
            form
        };
        
        // Send request
        let response = client
            .post("https://api.openai.com/v1/audio/transcriptions")
            .header("Authorization", format!("Bearer {}", api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| VoiceError::RequestError(e.to_string()))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(VoiceError::ApiError(error_text));
        }
        
        // Parse response
        let result: serde_json::Value = response.json()
            .await
            .map_err(|e| VoiceError::ParseError(e.to_string()))?;
        
        let text = result["text"].as_str().unwrap_or("").to_string();
        let language = result["language"].as_str().map(|s| s.to_string());
        let duration = result["duration"].as_f64();
        
        Ok(TranscriptionResult {
            text: text.clone(),
            language,
            duration_seconds: duration,
            word_count: text.split_whitespace().count(),
            tokens_used: None,
            model: format!("whisper-{}", self.config.model),
        })
    }
    
    /// Transcribe via whisper.cpp local
    async fn transcribe_via_whisper_cpp(&self, audio_path: &Path, whisper_path: &Path) -> Result<TranscriptionResult, VoiceError> {
        let output_path = audio_path.with_extension("txt");
        
        // Run whisper.cpp
        let status = tokio::process::Command::new(whisper_path)
            .arg("--model")
            .arg(self.config.model.to_string())
            .arg("--output-file")
            .arg(&output_path)
            .arg(audio_path)
            .status()
            .await
            .map_err(|e| VoiceError::ProcessError(e.to_string()))?;
        
        if !status.success() {
            return Err(VoiceError::ProcessError("whisper.cpp failed".to_string()));
        }
        
        // Read output
        let text = std::fs::read_to_string(&output_path)
            .map_err(|e| VoiceError::IoError(e.to_string()))?;
        
        Ok(TranscriptionResult {
            text: text.clone(),
            language: self.config.language.clone(),
            duration_seconds: None,
            word_count: text.split_whitespace().count(),
            tokens_used: None,
            model: format!("whisper.cpp-{}", self.config.model),
        })
    }
    
    /// Fallback transcription (mock for testing)
    async fn transcribe_fallback(&self, _audio_path: &Path) -> Result<TranscriptionResult, VoiceError> {
        // No API key and no local whisper - return error
        Err(VoiceError::NoTranscriptionMethod)
    }
    
    /// Get supported formats
    pub fn supported_formats(&self) -> &[String] {
        &self.config.supported_formats
    }
    
    /// Check if a format is supported
    pub fn is_format_supported(&self, format: &str) -> bool {
        self.config.supported_formats.contains(&format.to_lowercase())
    }
}

/// Voice transcription errors
#[derive(Debug, Clone)]
pub enum VoiceError {
    /// File not found
    FileNotFound(PathBuf),
    /// Unsupported format
    UnsupportedFormat(String),
    /// File too large
    FileTooLarge { size_mb: f64, max_mb: f64 },
    /// IO error
    IoError(String),
    /// HTTP request error
    RequestError(String),
    /// API error
    ApiError(String),
    /// Parse error
    ParseError(String),
    /// Process error
    ProcessError(String),
    /// No transcription method available
    NoTranscriptionMethod,
    /// No HTTP client
    NoClient,
}

impl std::fmt::Display for VoiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VoiceError::FileNotFound(path) => write!(f, "File not found: {}", path.display()),
            VoiceError::UnsupportedFormat(format) => write!(f, "Unsupported format: {}", format),
            VoiceError::FileTooLarge { size_mb, max_mb } => {
                write!(f, "File too large: {:.2}MB (max: {:.2}MB)", size_mb, max_mb)
            }
            VoiceError::IoError(e) => write!(f, "IO error: {}", e),
            VoiceError::RequestError(e) => write!(f, "Request error: {}", e),
            VoiceError::ApiError(e) => write!(f, "API error: {}", e),
            VoiceError::ParseError(e) => write!(f, "Parse error: {}", e),
            VoiceError::ProcessError(e) => write!(f, "Process error: {}", e),
            VoiceError::NoTranscriptionMethod => write!(f, "No transcription method available (set API key or whisper.cpp path)"),
            VoiceError::NoClient => write!(f, "No HTTP client available"),
        }
    }
}

impl std::error::Error for VoiceError {}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_voice_config_default() {
        let config = VoiceConfig::default();
        assert!(config.enabled);
        assert_eq!(config.model, VoiceModel::Small);
        assert!(config.supported_formats.contains(&"mp3".to_string()));
    }
    
    #[test]
    fn test_voice_model_display() {
        assert_eq!(VoiceModel::Tiny.to_string(), "tiny");
        assert_eq!(VoiceModel::Base.to_string(), "base");
        assert_eq!(VoiceModel::Small.to_string(), "small");
        assert_eq!(VoiceModel::Medium.to_string(), "medium");
        assert_eq!(VoiceModel::Large.to_string(), "large");
        assert_eq!(VoiceModel::Turbo.to_string(), "turbo");
    }
    
    #[test]
    fn test_voice_processor_new() {
        let config = VoiceConfig::default();
        let processor = VoiceProcessor::new(config);
        assert!(processor.http_client.is_some());
    }
    
    #[test]
    fn test_supported_formats() {
        let processor = VoiceProcessor::default_config();
        assert!(processor.is_format_supported("mp3"));
        assert!(processor.is_format_supported("wav"));
        assert!(processor.is_format_supported("webm"));
        assert!(!processor.is_format_supported("xyz"));
    }
    
    #[test]
    fn test_transcription_result() {
        let result = TranscriptionResult {
            text: "Hello world this is a test".to_string(),
            language: Some("en".to_string()),
            duration_seconds: Some(5.0),
            word_count: 6,
            tokens_used: None,
            model: "whisper-small".to_string(),
        };
        
        assert_eq!(result.word_count, 6);
        assert_eq!(result.language, Some("en".to_string()));
    }
    
    #[test]
    fn test_voice_error_display() {
        let error = VoiceError::FileNotFound(PathBuf::from("/tmp/test.mp3"));
        assert!(error.to_string().contains("not found"));
        
        let error = VoiceError::FileTooLarge { size_mb: 50.0, max_mb: 25.0 };
        assert!(error.to_string().contains("too large"));
        
        let error = VoiceError::UnsupportedFormat("avi".to_string());
        assert!(error.to_string().contains("Unsupported"));
    }
    
    #[tokio::test]
    async fn test_transcribe_file_not_found() {
        let processor = VoiceProcessor::default_config();
        let result = processor.transcribe(Path::new("/nonexistent/file.mp3")).await;
        assert!(result.is_err());
    }
    
    #[test]
    fn test_voice_config_with_api() {
        let config = VoiceConfig {
            api_key: Some("sk-test".to_string()),
            ..Default::default()
        };
        
        assert!(config.api_key.is_some());
    }
}