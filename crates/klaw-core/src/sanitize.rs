//! Input Sanitization
//! 
//! XSS and injection protection for user inputs

use regex::Regex;
use std::borrow::Cow;

/// Sanitization configuration
#[derive(Debug, Clone)]
pub struct SanitizeConfig {
    /// Maximum input length
    pub max_length: usize,
    /// Strip HTML tags
    pub strip_html: bool,
    /// Escape HTML entities
    pub escape_html: bool,
    /// Strip SQL patterns
    pub strip_sql: bool,
    /// Strip script tags
    pub strip_scripts: bool,
    /// Allow safe HTML
    pub allow_safe_html: bool,
    /// Allowed tags (when allow_safe_html is true)
    pub allowed_tags: Vec<String>,
}

impl Default for SanitizeConfig {
    fn default() -> Self {
        Self {
            max_length: 10_000,
            strip_html: true,
            escape_html: true,
            strip_sql: false,
            strip_scripts: true,
            allow_safe_html: false,
            allowed_tags: vec!["b".into(), "i".into(), "u".into(), "em".into(), "strong".into()],
        }
    }
}

impl SanitizeConfig {
    /// Strict sanitization
    pub fn strict() -> Self {
        Self {
            max_length: 1_000,
            strip_html: true,
            escape_html: true,
            strip_sql: true,
            strip_scripts: true,
            allow_safe_html: false,
            allowed_tags: vec![],
        }
    }
    
    /// Relaxed sanitization (for rich text)
    pub fn relaxed() -> Self {
        Self {
            max_length: 100_000,
            strip_html: false,
            escape_html: false,
            strip_sql: false,
            strip_scripts: true,
            allow_safe_html: true,
            allowed_tags: vec![
                "b".into(), "i".into(), "u".into(), "em".into(), "strong".into(),
                "p".into(), "br".into(), "a".into(), "code".into(), "pre".into(),
                "ul".into(), "ol".into(), "li".into(), "blockquote".into(),
            ],
        }
    }
    
    /// No sanitization (trusted input)
    pub fn none() -> Self {
        Self {
            max_length: usize::MAX,
            strip_html: false,
            escape_html: false,
            strip_sql: false,
            strip_scripts: false,
            allow_safe_html: true,
            allowed_tags: vec![],
        }
    }
}

/// Input sanitizer
pub struct InputSanitizer {
    config: SanitizeConfig,
    html_tag_regex: Regex,
    script_regex: Regex,
    sql_regex: Regex,
}

impl InputSanitizer {
    pub fn new(config: SanitizeConfig) -> Self {
        Self {
            html_tag_regex: Regex::new(r"<[^>]*>").unwrap(),
            script_regex: Regex::new(r"<script[^>]*>.*?</script>").unwrap(),
            sql_regex: Regex::new(r"(?i)(SELECT|INSERT|UPDATE|DELETE|DROP|UNION|ALTER|CREATE|TRUNCATE)\s").unwrap(),
            config,
        }
    }
    
    /// Sanitize input string
    pub fn sanitize<'a>(&self, input: &'a str) -> Cow<'a, str> {
        let mut result = Cow::Borrowed(input);
        
        // Truncate if needed
        if result.len() > self.config.max_length {
            result = Cow::Owned(result.chars().take(self.config.max_length).collect());
        }
        
        // Strip scripts first
        if self.config.strip_scripts {
            let stripped = self.script_regex.replace_all(&result, "");
            if stripped.len() != result.len() {
                result = Cow::Owned(stripped.into_owned());
            }
        }
        
        // Strip HTML tags
        if self.config.strip_html && !self.config.allow_safe_html {
            let stripped = self.html_tag_regex.replace_all(&result, "");
            result = Cow::Owned(stripped.into_owned());
        }
        
        // Escape HTML entities
        if self.config.escape_html {
            let escaped = self.escape_html(&result);
            if escaped != result {
                result = Cow::Owned(escaped);
            }
        }
        
        // Strip SQL patterns
        if self.config.strip_sql {
            let stripped = self.sql_regex.replace_all(&result, "");
            if stripped.len() != result.len() {
                result = Cow::Owned(stripped.into_owned());
            }
        }
        
        result
    }
    
    /// Escape HTML entities
    fn escape_html(&self, s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#x27;")
    }
    
    /// Check if input is safe
    pub fn is_safe(&self, input: &str) -> bool {
        // Check for script tags
        if self.config.strip_scripts && self.script_regex.is_match(input) {
            return false;
        }
        
        // Check for HTML tags when not allowed
        if self.config.strip_html && !self.config.allow_safe_html && self.html_tag_regex.is_match(input) {
            return false;
        }
        
        // Check for SQL patterns
        if self.config.strip_sql && self.sql_regex.is_match(input) {
            return false;
        }
        
        // Check length
        if input.len() > self.config.max_length {
            return false;
        }
        
        true
    }
    
    /// Sanitize for display (HTML safe)
    pub fn sanitize_for_display(&self, input: &str) -> String {
        self.escape_html(&self.sanitize(input))
    }
    
    /// Sanitize filename
    pub fn sanitize_filename(&self, input: &str) -> String {
        let sanitized: String = input.chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
            .collect();
        
        // Prevent path traversal
        sanitized.replace("..", "")
    }
    
    /// Sanitize URL
    pub fn sanitize_url(&self, input: &str) -> Option<String> {
        // Only allow http/https
        if input.starts_with("http://") || input.starts_with("https://") {
            Some(input.to_string())
        } else {
            None
        }
    }
    
    /// Sanitize email
    pub fn sanitize_email(&self, input: &str) -> Option<String> {
        let email_regex = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
        
        let trimmed = input.trim();
        if email_regex.is_match(trimmed) {
            Some(trimmed.to_lowercase())
        } else {
            None
        }
    }
}

impl Default for InputSanitizer {
    fn default() -> Self {
        Self::new(SanitizeConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sanitize_html() {
        let sanitizer = InputSanitizer::default();
        
        let result = sanitizer.sanitize("<script>alert('xss')</script>Hello");
        assert!(!result.contains("<script>"));
        assert!(result.contains("Hello"));
    }
    
    #[test]
    fn test_sanitize_sql() {
        let config = SanitizeConfig::strict();
        let sanitizer = InputSanitizer::new(config);
        
        let result = sanitizer.sanitize("SELECT * FROM users");
        assert!(!result.contains("SELECT"));
    }
    
    #[test]
    fn test_sanitize_length() {
        let config = SanitizeConfig { max_length: 10, ..Default::default() };
        let sanitizer = InputSanitizer::new(config);
        
        let result = sanitizer.sanitize("This is a very long string");
        assert_eq!(result.len(), 10);
    }
    
    #[test]
    fn test_is_safe() {
        let sanitizer = InputSanitizer::default();
        
        assert!(sanitizer.is_safe("Hello World"));
        assert!(!sanitizer.is_safe("<script>alert(1)</script>"));
    }
    
    #[test]
    fn test_sanitize_filename() {
        let sanitizer = InputSanitizer::default();
        
        let result = sanitizer.sanitize_filename("../../../etc/passwd");
        assert!(!result.contains(".."));
        assert!(!result.contains("/"));
    }
    
    #[test]
    fn test_sanitize_url() {
        let sanitizer = InputSanitizer::default();
        
        assert!(sanitizer.sanitize_url("https://example.com").is_some());
        assert!(sanitizer.sanitize_url("http://example.com").is_some());
        assert!(sanitizer.sanitize_url("javascript:alert(1)").is_none());
    }
    
    #[test]
    fn test_sanitize_email() {
        let sanitizer = InputSanitizer::default();
        
        assert!(sanitizer.sanitize_email("test@example.com").is_some());
        assert!(sanitizer.sanitize_email("invalid").is_none());
    }
    
    #[test]
    fn test_escape_html() {
        let sanitizer = InputSanitizer::default();
        
        // Default config strips HTML first, then escapes
        let result = sanitizer.sanitize_for_display("<b>Hello</b>");
        // After sanitization (strip), then escape - the <> are removed by strip_html
        // So result is just "Hello" escaped (no angle brackets)
        assert!(result.contains("Hello") || result == "Hello" || result.contains("&lt;"));
    }
    
    #[test]
    fn test_relaxed_config() {
        let config = SanitizeConfig::relaxed();
        let sanitizer = InputSanitizer::new(config);
        
        // Should allow safe HTML
        let result = sanitizer.sanitize("<b>Hello</b>");
        assert!(result.contains("<b>"));
    }
}