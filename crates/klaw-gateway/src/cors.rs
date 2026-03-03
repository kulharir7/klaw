//! CORS Configuration
//! 
//! Cross-Origin Resource Sharing settings for the gateway

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// CORS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CorsConfig {
    /// Allowed origins
    pub allowed_origins: Vec<String>,
    /// Allowed methods
    pub allowed_methods: Vec<String>,
    /// Allowed headers
    pub allowed_headers: Vec<String>,
    /// Exposed headers
    pub exposed_headers: Vec<String>,
    /// Allow credentials
    pub allow_credentials: bool,
    /// Max age in seconds
    pub max_age_seconds: u64,
    /// Allow all origins
    pub allow_all_origins: bool,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: vec!["http://localhost:3000".to_string()],
            allowed_methods: vec![
                "GET".to_string(),
                "POST".to_string(),
                "PUT".to_string(),
                "DELETE".to_string(),
                "OPTIONS".to_string(),
                "PATCH".to_string(),
            ],
            allowed_headers: vec![
                "Authorization".to_string(),
                "Content-Type".to_string(),
                "X-Requested-With".to_string(),
                "Accept".to_string(),
                "Origin".to_string(),
                "Access-Control-Request-Method".to_string(),
                "Access-Control-Request-Headers".to_string(),
            ],
            exposed_headers: vec![
                "X-Total-Count".to_string(),
                "X-Page".to_string(),
                "X-Per-Page".to_string(),
            ],
            allow_credentials: true,
            max_age_seconds: 86400,
            allow_all_origins: false,
        }
    }
}

impl CorsConfig {
    /// Create new CORS config
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Allow all origins (development mode)
    pub fn permissive() -> Self {
        Self {
            allowed_origins: vec![],
            allowed_methods: vec![
                "GET".to_string(),
                "POST".to_string(),
                "PUT".to_string(),
                "DELETE".to_string(),
                "OPTIONS".to_string(),
                "PATCH".to_string(),
                "HEAD".to_string(),
            ],
            allowed_headers: vec!["*".to_string()],
            exposed_headers: vec!["*".to_string()],
            allow_credentials: false,
            max_age_seconds: 86400,
            allow_all_origins: true,
        }
    }
    
    /// Strict CORS (production)
    pub fn strict() -> Self {
        Self {
            allowed_origins: vec![],
            allowed_methods: vec!["GET".to_string(), "POST".to_string()],
            allowed_headers: vec![
                "Authorization".to_string(),
                "Content-Type".to_string(),
            ],
            exposed_headers: vec![],
            allow_credentials: false,
            max_age_seconds: 3600,
            allow_all_origins: false,
        }
    }
    
    /// Add allowed origin
    pub fn with_origin(mut self, origin: &str) -> Self {
        self.allowed_origins.push(origin.to_string());
        self
    }
    
    /// Add allowed method
    pub fn with_method(mut self, method: &str) -> Self {
        self.allowed_methods.push(method.to_string());
        self
    }
    
    /// Add allowed header
    pub fn with_header(mut self, header: &str) -> Self {
        self.allowed_headers.push(header.to_string());
        self
    }
    
    /// Check if origin is allowed
    pub fn is_origin_allowed(&self, origin: &str) -> bool {
        if self.allow_all_origins {
            return true;
        }
        self.allowed_origins.iter().any(|o| {
            o == origin || 
            o == "*" ||
            self.matches_pattern(origin, o)
        })
    }
    
    /// Check if method is allowed
    pub fn is_method_allowed(&self, method: &str) -> bool {
        self.allowed_methods.iter().any(|m| m == method || m == "*")
    }
    
    /// Check if header is allowed
    pub fn is_header_allowed(&self, header: &str) -> bool {
        self.allowed_headers.iter().any(|h| {
            h == header || 
            h == "*" ||
            h.to_lowercase() == header.to_lowercase()
        })
    }
    
    /// Get Access-Control-Allow-Origin header value
    pub fn allow_origin(&self, request_origin: Option<&str>) -> Option<String> {
        if self.allow_all_origins {
            return Some("*".to_string());
        }
        
        request_origin.and_then(|origin| {
            if self.is_origin_allowed(origin) {
                Some(origin.to_string())
            } else {
                None
            }
        })
    }
    
    /// Match origin against pattern
    fn matches_pattern(&self, origin: &str, pattern: &str) -> bool {
        if pattern.starts_with("*.") {
            let domain = &pattern[2..];
            origin.ends_with(domain)
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cors_default() {
        let cors = CorsConfig::default();
        assert!(!cors.allow_all_origins);
        assert!(cors.allow_credentials);
        assert!(cors.allowed_methods.contains(&"GET".to_string()));
    }
    
    #[test]
    fn test_cors_permissive() {
        let cors = CorsConfig::permissive();
        assert!(cors.allow_all_origins);
        assert!(cors.is_origin_allowed("http://evil.com"));
    }
    
    #[test]
    fn test_cors_strict() {
        let cors = CorsConfig::strict();
        assert!(!cors.allow_all_origins);
        assert!(!cors.allow_credentials);
    }
    
    #[test]
    fn test_origin_allowed() {
        let cors = CorsConfig::default();
        assert!(cors.is_origin_allowed("http://localhost:3000"));
        assert!(!cors.is_origin_allowed("http://evil.com"));
    }
    
    #[test]
    fn test_method_allowed() {
        let cors = CorsConfig::default();
        assert!(cors.is_method_allowed("GET"));
        assert!(cors.is_method_allowed("POST"));
        assert!(cors.is_method_allowed("DELETE"));
    }
    
    #[test]
    fn test_builder() {
        let cors = CorsConfig::new()
            .with_origin("http://example.com")
            .with_method("PUT")
            .with_header("X-Custom");
        
        assert!(cors.is_origin_allowed("http://example.com"));
        assert!(cors.is_method_allowed("PUT"));
        assert!(cors.is_header_allowed("X-Custom"));
    }
    
    #[test]
    fn test_wildcard_domain() {
        let cors = CorsConfig::new()
            .with_origin("*.example.com");
        
        assert!(cors.is_origin_allowed("http://api.example.com"));
        assert!(cors.is_origin_allowed("https://app.example.com"));
        assert!(!cors.is_origin_allowed("http://example.org"));
    }
    
    #[test]
    fn test_allow_origin_header() {
        let cors = CorsConfig::default();
        
        let value = cors.allow_origin(Some("http://localhost:3000"));
        assert_eq!(value, Some("http://localhost:3000".to_string()));
        
        let value = cors.allow_origin(Some("http://evil.com"));
        assert_eq!(value, None);
        
        let permissive = CorsConfig::permissive();
        let value = permissive.allow_origin(None);
        assert_eq!(value, Some("*".to_string()));
    }
}