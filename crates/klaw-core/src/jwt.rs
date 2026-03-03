//! JWT Authentication
//! 
//! Token-based authentication for the gateway and sessions

use serde::{Deserialize, Serialize};
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey, Algorithm};
use std::time::{SystemTime, UNIX_EPOCH};

/// JWT Claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user/session ID)
    pub sub: String,
    /// Issuer
    pub iss: String,
    /// Audience
    pub aud: String,
    /// Issued at (timestamp)
    pub iat: u64,
    /// Expiration (timestamp)
    pub exp: u64,
    /// Custom claims
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_key: Option<String>,
}

/// JWT Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct JwtConfig {
    /// Secret key for signing
    pub secret: String,
    /// Token expiration in seconds
    pub expiration_seconds: u64,
    /// Issuer
    pub issuer: String,
    /// Audience
    pub audience: String,
    /// Algorithm
    #[serde(default = "default_algorithm")]
    pub algorithm: String,
    /// Refresh token expiration in seconds
    pub refresh_expiration_seconds: u64,
}

fn default_algorithm() -> String { "HS256".to_string() }

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: "klaw-default-secret-change-in-production".to_string(),
            expiration_seconds: 3600, // 1 hour
            issuer: "klaw".to_string(),
            audience: "klaw-users".to_string(),
            algorithm: default_algorithm(),
            refresh_expiration_seconds: 7 * 24 * 3600, // 7 days
        }
    }
}

impl JwtConfig {
    /// Create with custom secret
    pub fn with_secret(secret: &str) -> Self {
        Self {
            secret: secret.to_string(),
            ..Default::default()
        }
    }
    
    /// Create development config (insecure)
    pub fn dev() -> Self {
        Self::default()
    }
    
    /// Create production config
    pub fn production(secret: &str, issuer: &str) -> Self {
        Self {
            secret: secret.to_string(),
            expiration_seconds: 3600,
            issuer: issuer.to_string(),
            audience: "users".to_string(),
            algorithm: "HS256".to_string(),
            refresh_expiration_seconds: 86400, // 1 day
        }
    }
}

/// JWT Handler
pub struct JwtHandler {
    config: JwtConfig,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

impl JwtHandler {
    /// Create new JWT handler
    pub fn new(config: JwtConfig) -> Self {
        let decoding_key = DecodingKey::from_secret(config.secret.as_bytes());
        let encoding_key = EncodingKey::from_secret(config.secret.as_bytes());
        
        Self {
            config,
            encoding_key,
            decoding_key,
        }
    }
    
    /// Create from secret (convenience)
    pub fn from_secret(secret: &str) -> Self {
        Self::new(JwtConfig::with_secret(secret))
    }
    
    /// Generate token
    pub fn generate(&self, subject: &str) -> Result<String, JwtError> {
        self.generate_with_claims(subject, None, None)
    }
    
    /// Generate token with extra claims
    pub fn generate_with_claims(
        &self,
        subject: &str,
        agent_id: Option<&str>,
        session_key: Option<&str>,
    ) -> Result<String, JwtError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| JwtError::TimeError)?
            .as_secs();
        
        let claims = Claims {
            sub: subject.to_string(),
            iss: self.config.issuer.clone(),
            aud: self.config.audience.clone(),
            iat: now,
            exp: now + self.config.expiration_seconds,
            role: Some("user".to_string()),
            agent_id: agent_id.map(|s| s.to_string()),
            session_key: session_key.map(|s| s.to_string()),
        };
        
        let header = Header::new(Algorithm::HS256);
        encode(&header, &claims, &self.encoding_key)
            .map_err(JwtError::EncodeError)
    }
    
    /// Validate token
    pub fn validate(&self, token: &str) -> Result<Claims, JwtError> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_audience(&[&self.config.audience]);
        validation.set_issuer(&[&self.config.issuer]);
        decode::<Claims>(token, &self.decoding_key, &validation)
            .map(|data| data.claims)
            .map_err(JwtError::DecodeError)
    }
    
    /// Validate and check issuer
    pub fn validate_with_issuer(&self, token: &str) -> Result<Claims, JwtError> {
        self.validate(token)
    }
    
    /// Generate refresh token
    pub fn generate_refresh(&self, subject: &str) -> Result<String, JwtError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| JwtError::TimeError)?
            .as_secs();
        
        let claims = Claims {
            sub: subject.to_string(),
            iss: self.config.issuer.clone(),
            aud: self.config.audience.clone(),
            iat: now,
            exp: now + self.config.refresh_expiration_seconds,
            role: Some("refresh".to_string()),
            agent_id: None,
            session_key: None,
        };
        
        let header = Header::new(Algorithm::HS256);
        encode(&header, &claims, &self.encoding_key)
            .map_err(JwtError::EncodeError)
    }
    
    /// Check if token is expired
    pub fn is_expired(&self, token: &str) -> bool {
        if let Ok(claims) = self.validate(token) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            claims.exp < now
        } else {
            true
        }
    }
    
    /// Get token expiration time
    pub fn get_expiration(&self, token: &str) -> Option<u64> {
        self.validate(token).ok().map(|c| c.exp)
    }
    
    /// Refresh token
    pub fn refresh(&self, token: &str) -> Result<String, JwtError> {
        let claims = self.validate(token)?;
        
        if claims.role.as_deref() != Some("refresh") {
            return Err(JwtError::InvalidTokenType);
        }
        
        self.generate(&claims.sub)
    }
}

/// JWT Error
#[derive(Debug, Clone)]
pub enum JwtError {
    /// Encoding error
    EncodeError(jsonwebtoken::errors::Error),
    /// Decoding error
    DecodeError(jsonwebtoken::errors::Error),
    /// Invalid issuer
    InvalidIssuer,
    /// Invalid token type
    InvalidTokenType,
    /// Time error
    TimeError,
}

impl std::fmt::Display for JwtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JwtError::EncodeError(e) => write!(f, "JWT encode error: {}", e),
            JwtError::DecodeError(e) => write!(f, "JWT decode error: {}", e),
            JwtError::InvalidIssuer => write!(f, "Invalid issuer"),
            JwtError::InvalidTokenType => write!(f, "Invalid token type"),
            JwtError::TimeError => write!(f, "Time error"),
        }
    }
}

impl std::error::Error for JwtError {}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_generate_and_validate() {
        let handler = JwtHandler::from_secret("test-secret");
        
        let token = handler.generate("user-123").unwrap();
        assert!(!token.is_empty());
        
        let claims = handler.validate(&token).unwrap();
        assert_eq!(claims.sub, "user-123");
    }
    
    #[test]
    fn test_token_with_claims() {
        let handler = JwtHandler::from_secret("test-secret");
        
        let token = handler.generate_with_claims(
            "user-123",
            Some("agent-1"),
            Some("session-1")
        ).unwrap();
        
        let claims = handler.validate(&token).unwrap();
        assert_eq!(claims.agent_id, Some("agent-1".to_string()));
        assert_eq!(claims.session_key, Some("session-1".to_string()));
    }
    
    #[test]
    fn test_refresh_token() {
        let handler = JwtHandler::from_secret("test-secret");
        
        let refresh_token = handler.generate_refresh("user-123").unwrap();
        let new_token = handler.refresh(&refresh_token).unwrap();
        
        let claims = handler.validate(&new_token).unwrap();
        assert_eq!(claims.sub, "user-123");
    }
    
    #[test]
    fn test_invalid_token() {
        let handler = JwtHandler::from_secret("test-secret");
        
        let result = handler.validate("invalid-token");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_issuer_validation() {
        let handler = JwtHandler::from_secret("test-secret");
        
        let token = handler.generate("user-123").unwrap();
        let claims = handler.validate_with_issuer(&token).unwrap();
        
        assert_eq!(claims.iss, "klaw");
    }
    
    #[test]
    fn test_config() {
        let config = JwtConfig::production("my-secret", "myapp");
        assert_eq!(config.issuer, "myapp");
        assert_eq!(config.expiration_seconds, 3600);
    }
}