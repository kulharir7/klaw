use klaw_core::types::{ChatType, InboundMessage};
use serde_json::Value;
use tracing::info;

pub struct WebhookHandler {
    pub path: String,
    pub secret: Option<String>,
}

impl WebhookHandler {
    pub fn new(path: &str, secret: Option<String>) -> Self {
        Self {
            path: path.to_string(),
            secret,
        }
    }

    /// Validate webhook signature (HMAC-SHA256)
    /// Expects signature in format "sha256=<hex>"
    pub fn validate(&self, body: &[u8], signature: &str) -> bool {
        let Some(ref secret) = self.secret else {
            return true; // No secret = accept all
        };

        let sig_hex = signature.strip_prefix("sha256=").unwrap_or(signature);

        // Simple hash for validation (placeholder — use proper HMAC-SHA256 in production)
        let mut hasher_input = secret.as_bytes().to_vec();
        hasher_input.extend_from_slice(body);
        let expected = simple_hash_hex(&hasher_input);

        expected == sig_hex
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

/// Simple hash placeholder (NOT cryptographic — replace with HMAC-SHA256)
fn simple_hash_hex(data: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325; // FNV-1a offset basis
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", hash)
}
