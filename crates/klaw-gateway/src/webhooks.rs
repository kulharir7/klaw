use klaw_core::types::{ChatType, InboundMessage};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use hmac::{Hmac, Mac};
use tracing::{info, warn};

type HmacSha256 = Hmac<Sha256>;

pub struct WebhookHandler {
    pub path: String,
    pub secret: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// Webhook path (e.g., "/webhook/telegram")
    pub path: String,
    /// Secret for signature validation
    pub secret: Option<String>,
    /// Allowed IP addresses (CIDR notation)
    #[serde(default)]
    pub allowed_ips: Vec<String>,
    /// Rate limit: max requests per minute
    #[serde(default = "default_rate_limit")]
    pub rate_limit: u32,
    /// Timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

fn default_rate_limit() -> u32 { 100 }
fn default_timeout() -> u64 { 30 }

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            path: "/webhook".to_string(),
            secret: None,
            allowed_ips: vec![],
            rate_limit: default_rate_limit(),
            timeout_seconds: default_timeout(),
        }
    }
}

impl WebhookHandler {
    pub fn new(path: &str, secret: Option<String>) -> Self {
        Self {
            path: path.to_string(),
            secret,
        }
    }

    /// Create from config
    pub fn from_config(config: WebhookConfig) -> Self {
        Self {
            path: config.path,
            secret: config.secret,
        }
    }

    /// Validate webhook signature using HMAC-SHA256
    /// Expects signature in format "sha256=<hex>"
    pub fn validate(&self, body: &[u8], signature: &str) -> bool {
        let Some(ref secret) = self.secret else {
            return true; // No secret = accept all (insecure)
        };

        let sig_hex = signature.strip_prefix("sha256=").unwrap_or(signature);
        
        // Use proper HMAC-SHA256
        match HmacSha256::new_from_slice(secret.as_bytes()) {
            Ok(mut mac) => {
                mac.update(body);
                let result = mac.finalize();
                let expected = hex::encode(result.into_bytes());
                constant_time_eq::eq_hex(sig_hex.as_bytes(), expected.as_bytes())
            }
            Err(_) => {
                warn!("Invalid secret for HMAC");
                false
            }
        }
    }

    /// Validate IP address against allowed list
    pub fn validate_ip(&self, ip: &str, allowed: &[String]) -> bool {
        if allowed.is_empty() {
            return true; // No restrictions
        }
        
        // Simple CIDR check
        for cidr in allowed {
            if cidr == ip {
                return true;
            }
            if cidr.ends_with("/*") {
                let prefix = cidr.trim_end_matches("/*");
                if ip.starts_with(prefix) {
                    return true;
                }
            }
            if cidr.contains('/') {
                let parts: Vec<&str> = cidr.split('/').collect();
                if parts.len() == 2 && ip.starts_with(parts[0]) {
                    return true;
                }
            }
        }
        
        warn!("IP {} not in allowed list", ip);
        false
    }

    /// Process inbound webhook body into an InboundMessage
    pub fn process(&self, body: Value) -> Option<InboundMessage> {
        let text = body.get("text")
            .or_else(|| body.get("message"))
            .or_else(|| body.get("content"))
            .and_then(|v| v.as_str())?;

        let sender = body.get("sender")
            .or_else(|| body.get("user"))
            .or_else(|| body.get("from"))
            .and_then(|v| v.as_str())
            .unwrap_or("webhook");

        let channel_id = body.get("channel")
            .or_else(|| body.get("chat_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("webhook");

        info!("Webhook message from {}: {}", sender, text);

        Some(InboundMessage {
            id: uuid::Uuid::new_v4().to_string(),
            channel: "webhook".to_string(),
            chat_id: channel_id.to_string(),
            sender_id: sender.to_string(),
            sender_name: Some(sender.to_string()),
            text: Some(text.to_string()),
            reply_to: None,
            media: None,
            chat_type: ChatType::Direct,
            timestamp: chrono::Utc::now(),
        })
    }
}

/// Placeholder implementation (use constant_time_eq crate in production)
mod constant_time_eq {
    pub fn eq_hex(a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        let mut result: u8 = 0;
        for (x, y) in a.iter().zip(b.iter()) {
            result |= x ^ y;
        }
        result == 0
    }
}
