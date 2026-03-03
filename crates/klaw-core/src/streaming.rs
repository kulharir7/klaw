//! Block/Chunk streaming configuration
//! Controls how messages are delivered in chunks for better UX

use serde::{Deserialize, Serialize};

/// Streaming configuration per agent
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StreamingConfig {
    /// Enable streaming (default: true)
    pub enabled: bool,
    
    /// Streaming mode: "token", "block", "sentence"
    pub mode: StreamingMode,
    
    /// Chunk size in characters for block mode
    pub chunk_size: usize,
    
    /// Delay between chunks in milliseconds
    pub chunk_delay_ms: u64,
    
    /// Min words per chunk for sentence mode
    pub min_words_per_chunk: usize,
    
    /// Max words per chunk for sentence mode
    pub max_words_per_chunk: usize,
    
    /// Enable typing indicator while streaming
    pub show_typing: bool,
    
    /// Show progress indicator
    pub show_progress: bool,
    
    /// Prefix for each chunk (e.g., "..." or nothing)
    pub chunk_prefix: Option<String>,
    
    /// Suffix for each chunk (e.g., space or newline)
    pub chunk_suffix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamingMode {
    /// Stream token by token (real-time)
    Token,
    /// Stream in fixed-size blocks
    Block,
    /// Stream by sentences (smarter chunking)
    Sentence,
    /// Stream by paragraphs
    Paragraph,
}

impl Default for StreamingMode {
    fn default() -> Self {
        StreamingMode::Block
    }
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: StreamingMode::Block,
            chunk_size: 200,
            chunk_delay_ms: 50,
            min_words_per_chunk: 10,
            max_words_per_chunk: 30,
            show_typing: true,
            show_progress: false,
            chunk_prefix: None,
            chunk_suffix: Some(" ".to_string()),
        }
    }
}

impl StreamingConfig {
    /// Create a new streaming config with defaults
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Disable streaming entirely
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }
    
    /// Create token-by-token streaming config
    pub fn token_mode() -> Self {
        Self {
            enabled: true,
            mode: StreamingMode::Token,
            ..Self::default()
        }
    }
    
    /// Create block streaming config with custom chunk size
    pub fn block_mode(chunk_size: usize, delay_ms: u64) -> Self {
        Self {
            enabled: true,
            mode: StreamingMode::Block,
            chunk_size,
            chunk_delay_ms: delay_ms,
            ..Self::default()
        }
    }
    
    /// Create sentence streaming config
    pub fn sentence_mode() -> Self {
        Self {
            enabled: true,
            mode: StreamingMode::Sentence,
            ..Self::default()
        }
    }
    
    /// Chunk a message into blocks based on config
    pub fn chunk_message(&self, message: &str) -> Vec<String> {
        if !self.enabled {
            return vec![message.to_string()];
        }
        
        match self.mode {
            StreamingMode::Token => {
                // Character by character
                message.chars()
                    .map(|c| c.to_string())
                    .collect()
            }
            StreamingMode::Block => {
                // Fixed-size chunks
                self.chunk_by_size(message)
            }
            StreamingMode::Sentence => {
                // Sentence-based chunks
                self.chunk_by_sentence(message)
            }
            StreamingMode::Paragraph => {
                // Paragraph chunks
                message.split("\n\n")
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect()
            }
        }
    }
    
    fn chunk_by_size(&self, message: &str) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut current = String::new();
        let mut word_buffer = String::new();
        
        for word in message.split_whitespace() {
            if current.len() + word.len() + 1 > self.chunk_size && !current.is_empty() {
                chunks.push(current.trim().to_string());
                current = String::new();
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
        
        if !current.is_empty() {
            chunks.push(current.trim().to_string());
        }
        
        chunks
    }
    
    fn chunk_by_sentence(&self, message: &str) -> Vec<String> {
        // Split on sentence boundaries
        let sentences: Vec<&str> = message.split_inclusive(&['.', '!', '?', '\n'][..])
            .collect();
        
        let mut chunks = Vec::new();
        let mut current_chunk = String::new();
        let mut word_count = 0;
        
        for sentence in sentences {
            let trimmed = sentence.trim();
            if trimmed.is_empty() {
                continue;
            }
            
            let sentence_words = trimmed.split_whitespace().count();
            
            if word_count + sentence_words > self.max_words_per_chunk && !current_chunk.is_empty() {
                chunks.push(current_chunk.trim().to_string());
                current_chunk = String::new();
                word_count = 0;
            }
            
            if !current_chunk.is_empty() {
                current_chunk.push(' ');
            }
            current_chunk.push_str(trimmed);
            word_count += sentence_words;
            
            // Min words reached - flush
            if word_count >= self.min_words_per_chunk {
                chunks.push(current_chunk.trim().to_string());
                current_chunk = String::new();
                word_count = 0;
            }
        }
        
        if !current_chunk.is_empty() {
            chunks.push(current_chunk.trim().to_string());
        }
        
        chunks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = StreamingConfig::default();
        assert!(config.enabled);
        assert!(config.show_typing);
        assert_eq!(config.chunk_size, 200);
    }
    
    #[test]
    fn test_disabled_config() {
        let config = StreamingConfig::disabled();
        assert!(!config.enabled);
    }
    
    #[test]
    fn test_chunk_by_size() {
        let config = StreamingConfig::block_mode(20, 50);
        let message = "This is a test message that should be split into chunks based on size.";
        let chunks = config.chunk_message(message);
        
        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert!(chunk.len() <= 25); // Account for spaces
        }
    }
    
    #[test]
    fn test_chunk_by_sentence() {
        let config = StreamingConfig::sentence_mode();
        let message = "First sentence here. Second sentence follows. Third one ends it.";
        let chunks = config.chunk_message(message);
        
        assert!(!chunks.is_empty());
    }
    
    #[test]
    fn test_disabled_no_chunking() {
        let config = StreamingConfig::disabled();
        let message = "This is a test message.";
        let chunks = config.chunk_message(message);
        
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], message);
    }
}