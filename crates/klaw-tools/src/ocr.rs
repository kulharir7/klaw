//! OCR (Optical Character Recognition)
//!
//! Extract text from images using Tesseract OCR

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// OCR configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrConfig {
    /// Enable OCR
    pub enabled: bool,
    /// Tesseract binary path (if not in PATH)
    pub tesseract_path: Option<PathBuf>,
    /// Tessdata path (language data)
    pub tessdata_path: Option<PathBuf>,
    /// Primary language
    pub language: String,
    /// Additional languages
    pub additional_languages: Vec<String>,
    /// OCR Engine Mode (0-3)
    /// 0 = Original Tesseract only
    /// 1 = Neural net LSTM only
    /// 2 = Tesseract + LSTM
    /// 3 = Default (whatever is available)
    pub oem: u8,
    /// Page Segmentation Mode (0-13)
    /// 0 = Orientation and script detection only
    /// 1 = Automatic page segmentation
    /// 3 = Fully automatic page segmentation (default)
    /// 6 = Single uniform block of text
    pub psm: u8,
    /// DPI for images
    pub dpi: Option<u32>,
    /// Maximum image file size in MB
    pub max_file_size_mb: u32,
    /// Supported image formats
    pub supported_formats: Vec<String>,
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            tesseract_path: None,
            tessdata_path: None,
            language: "eng".to_string(),
            additional_languages: vec![],
            oem: 3,
            psm: 3,
            dpi: None,
            max_file_size_mb: 25,
            supported_formats: vec![
                "png".to_string(),
                "jpg".to_string(),
                "jpeg".to_string(),
                "gif".to_string(),
                "bmp".to_string(),
                "tiff".to_string(),
                "webp".to_string(),
                "pdf".to_string(),
            ],
        }
    }
}

/// OCR result with text and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    /// Extracted text
    pub text: String,
    /// Confidence score (0-100)
    pub confidence: Option<f32>,
    /// Words detected
    pub word_count: usize,
    /// Lines detected
    pub line_count: usize,
    /// Processing time in ms
    pub processing_time_ms: u64,
    /// Language detected/used
    pub language: String,
    /// Source file
    pub source_file: Option<String>,
}

/// OCR processor
pub struct OcrProcessor {
    config: OcrConfig,
}

impl OcrProcessor {
    /// Create new OCR processor
    pub fn new(config: OcrConfig) -> Self {
        Self { config }
    }
    
    /// Create with default config
    pub fn default_config() -> Self {
        Self::new(OcrConfig::default())
    }
    
    /// Extract text from image
    pub async fn extract_text(&self, image_path: &Path) -> Result<OcrResult, OcrError> {
        // Check if file exists
        if !image_path.exists() {
            return Err(OcrError::FileNotFound(image_path.to_path_buf()));
        }
        
        // Check file extension
        let ext = image_path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        
        if !self.config.supported_formats.contains(&ext.to_lowercase()) {
            return Err(OcrError::UnsupportedFormat(ext.to_string()));
        }
        
        // Check file size
        let metadata = std::fs::metadata(image_path)
            .map_err(|e| OcrError::IoError(e.to_string()))?;
        let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
        
        if size_mb > self.config.max_file_size_mb as f64 {
            return Err(OcrError::FileTooLarge {
                size_mb,
                max_mb: self.config.max_file_size_mb as f64,
            });
        }
        
        // Run OCR
        self.run_tesseract(image_path).await
    }
    
    /// Run Tesseract OCR
    async fn run_tesseract(&self, image_path: &Path) -> Result<OcrResult, OcrError> {
        let start = std::time::Instant::now();
        
        // Get Tesseract command
        let tesseract_cmd = self.config.tesseract_path.clone()
            .unwrap_or_else(|| PathBuf::from("tesseract"));
        
        // Build arguments
        let mut args = vec![
            image_path.to_string_lossy().to_string(),
            "stdout".to_string(), // Output to stdout
            "--oem".to_string(),
            self.config.oem.to_string(),
            "--psm".to_string(),
            self.config.psm.to_string(),
        ];
        
        // Add language
        let lang = if !self.config.additional_languages.is_empty() {
            format!("{}+{}", 
                self.config.language, 
                self.config.additional_languages.join("+")
            )
        } else {
            self.config.language.clone()
        };
        args.push("-l".to_string());
        args.push(lang.clone());
        
        // Add DPI if specified
        if let Some(dpi) = self.config.dpi {
            args.push("--dpi".to_string());
            args.push(dpi.to_string());
        }
        
        // Run Tesseract
        let output = tokio::process::Command::new(&tesseract_cmd)
            .args(&args)
            .output()
            .await
            .map_err(|e| OcrError::ProcessError(format!("Failed to run tesseract: {}", e)))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OcrError::ProcessError(format!("Tesseract failed: {}", stderr)));
        }
        
        // Parse output
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let processing_time_ms = start.elapsed().as_millis() as u64;
        
        // Count words and lines
        let word_count = text.split_whitespace().count();
        let line_count = text.lines().count();
        
        // Estimate confidence (Tesseract outputs confidence with --psm variants)
        let confidence = self.estimate_confidence(&text);
        
        Ok(OcrResult {
            text,
            confidence,
            word_count,
            line_count,
            processing_time_ms,
            language: lang,
            source_file: image_path.to_str().map(|s| s.to_string()),
        })
    }
    
    /// Estimate confidence based on text quality
    fn estimate_confidence(&self, text: &str) -> Option<f32> {
        if text.is_empty() {
            return Some(0.0);
        }
        
        // Count valid words vs garbage
        let words: Vec<&str> = text.split_whitespace().collect();
        let valid_words = words.iter().filter(|w| {
            w.chars().all(|c| c.is_alphabetic() || c.is_numeric() || ".,!?-'\"".contains(c))
        }).count();
        
        if words.is_empty() {
            return Some(0.0);
        }
        
        let ratio = valid_words as f32 / words.len() as f32;
        Some(ratio * 100.0)
    }
    
    /// Extract text with specific language
    pub async fn extract_text_with_language(
        &self, 
        image_path: &Path, 
        language: &str
    ) -> Result<OcrResult, OcrError> {
        let mut config = self.config.clone();
        config.language = language.to_string();
        
        let processor = Self::new(config);
        processor.extract_text(image_path).await
    }
    
    /// Extract text with bounding boxes (structured output)
    pub async fn extract_with_boxes(&self, image_path: &Path) -> Result<Vec<TextRegion>, OcrError> {
        // Run OCR with tsv output
        let output = self.run_tesseract_tsv(image_path).await?;
        
        // Parse TSV output
        self.parse_tsv_regions(&output)
    }
    
    /// Run Tesseract with TSV output
    async fn run_tesseract_tsv(&self, image_path: &Path) -> Result<String, OcrError> {
        let tesseract_cmd = self.config.tesseract_path.clone()
            .unwrap_or_else(|| PathBuf::from("tesseract"));
        
        let output = tokio::process::Command::new(&tesseract_cmd)
            .arg(image_path)
            .arg("stdout")
            .arg("--oem")
            .arg(self.config.oem.to_string())
            .arg("--psm")
            .arg(self.config.psm.to_string())
            .arg("-l")
            .arg(&self.config.language)
            .arg("tsv")
            .output()
            .await
            .map_err(|e| OcrError::ProcessError(e.to_string()))?;
        
        if !output.status.success() {
            return Err(OcrError::ProcessError("Tesseract TSV failed".to_string()));
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
    
    /// Parse TSV regions
    fn parse_tsv_regions(&self, tsv: &str) -> Result<Vec<TextRegion>, OcrError> {
        let mut regions = Vec::new();
        
        for line in tsv.lines().skip(1) { // Skip header
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 12 {
                let text = parts.get(11).unwrap_or(&"").to_string();
                if text.is_empty() {
                    continue;
                }
                
                regions.push(TextRegion {
                    text,
                    x: parts.get(6).and_then(|s| s.parse().ok()).unwrap_or(0),
                    y: parts.get(7).and_then(|s| s.parse().ok()).unwrap_or(0),
                    width: parts.get(8).and_then(|s| s.parse().ok()).unwrap_or(0),
                    height: parts.get(9).and_then(|s| s.parse().ok()).unwrap_or(0),
                    confidence: parts.get(10).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                });
            }
        }
        
        Ok(regions)
    }
    
    /// Check if Tesseract is available
    pub async fn is_tesseract_available(&self) -> bool {
        let tesseract_cmd = self.config.tesseract_path.clone()
            .unwrap_or_else(|| PathBuf::from("tesseract"));
        
        tokio::process::Command::new(&tesseract_cmd)
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    
    /// Get supported formats
    pub fn supported_formats(&self) -> &[String] {
        &self.config.supported_formats
    }
}

/// Text region with bounding box
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextRegion {
    /// Detected text
    pub text: String,
    /// X position
    pub x: i32,
    /// Y position
    pub y: i32,
    /// Width
    pub width: i32,
    /// Height
    pub height: i32,
    /// Confidence (0-100)
    pub confidence: f32,
}

/// OCR errors
#[derive(Debug, Clone)]
pub enum OcrError {
    /// File not found
    FileNotFound(PathBuf),
    /// Unsupported format
    UnsupportedFormat(String),
    /// File too large
    FileTooLarge { size_mb: f64, max_mb: f64 },
    /// IO error
    IoError(String),
    /// Process error
    ProcessError(String),
    /// Parse error
    ParseError(String),
}

impl std::fmt::Display for OcrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OcrError::FileNotFound(path) => write!(f, "File not found: {}", path.display()),
            OcrError::UnsupportedFormat(format) => write!(f, "Unsupported format: {}", format),
            OcrError::FileTooLarge { size_mb, max_mb } => {
                write!(f, "File too large: {:.2}MB (max: {:.2}MB)", size_mb, max_mb)
            }
            OcrError::IoError(e) => write!(f, "IO error: {}", e),
            OcrError::ProcessError(e) => write!(f, "Process error: {}", e),
            OcrError::ParseError(e) => write!(f, "Parse error: {}", e),
        }
    }
}

impl std::error::Error for OcrError {}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ocr_config_default() {
        let config = OcrConfig::default();
        assert!(config.enabled);
        assert_eq!(config.language, "eng");
        assert_eq!(config.oem, 3);
        assert_eq!(config.psm, 3);
        assert!(config.supported_formats.contains(&"png".to_string()));
    }
    
    #[test]
    fn test_ocr_processor_new() {
        let processor = OcrProcessor::default_config();
        assert!(processor.config.enabled);
    }
    
    #[test]
    fn test_supported_formats() {
        let processor = OcrProcessor::default_config();
        assert!(processor.config.supported_formats.contains(&"png".to_string()));
        assert!(processor.config.supported_formats.contains(&"jpg".to_string()));
        assert!(processor.config.supported_formats.contains(&"pdf".to_string()));
    }
    
    #[test]
    fn test_ocr_result() {
        let result = OcrResult {
            text: "Hello world".to_string(),
            confidence: Some(95.5),
            word_count: 2,
            line_count: 1,
            processing_time_ms: 100,
            language: "eng".to_string(),
            source_file: Some("test.png".to_string()),
        };
        
        assert_eq!(result.word_count, 2);
        assert_eq!(result.line_count, 1);
    }
    
    #[test]
    fn test_text_region() {
        let region = TextRegion {
            text: "test".to_string(),
            x: 10,
            y: 20,
            width: 100,
            height: 30,
            confidence: 98.5,
        };
        
        assert_eq!(region.text, "test");
        assert_eq!(region.confidence, 98.5);
    }
    
    #[test]
    fn test_ocr_error_display() {
        let error = OcrError::FileNotFound(PathBuf::from("/tmp/test.png"));
        assert!(error.to_string().contains("not found"));
        
        let error = OcrError::UnsupportedFormat("avi".to_string());
        assert!(error.to_string().contains("Unsupported"));
        
        let error = OcrError::FileTooLarge { size_mb: 50.0, max_mb: 25.0 };
        assert!(error.to_string().contains("too large"));
    }
    
    #[tokio::test]
    async fn test_extract_file_not_found() {
        let processor = OcrProcessor::default_config();
        let result = processor.extract_text(Path::new("/nonexistent/test.png")).await;
        assert!(result.is_err());
    }
    
    #[test]
    fn test_config_with_languages() {
        let config = OcrConfig {
            language: "eng".to_string(),
            additional_languages: vec!["spa".to_string(), "fra".to_string()],
            ..Default::default()
        };
        
        assert_eq!(config.additional_languages.len(), 2);
    }
}