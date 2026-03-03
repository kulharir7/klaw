//! WhatsApp Channel Placeholder
//! 
//! Full implementation requires external sidecar:
//! - Node.js: whatsapp-web.js or Baileys
//! - Go: whatsmeow
//! - Python: yowsup
//!
//! This module provides the interface spec and stub implementations.

use async_trait::async_trait;
use klaw_core::types::{ChatType, InboundMessage, OutboundMessage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

/// WhatsApp configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WhatsAppConfig {
    /// Phone number (E.164 format)
    pub phone_number: Option<String>,
    /// Business API credentials
    pub business_api_token: Option<String>,
    /// Business phone ID
    pub business_phone_id: Option<String>,
    /// Webhook verify token
    pub webhook_verify_token: Option<String>,
    /// Use WhatsApp Business API instead of personal
    pub use_business_api: bool,
    /// Sidecar URL (Node.js/Baileys)
    pub sidecar_url: Option<String>,
    /// Session data path
    pub session_path: Option<String>,
    /// Auto-reply to messages
    pub auto_reply: bool,
    /// Allowed contacts/groups
    pub allowed_contacts: Vec<String>,
    /// Blocked contacts/groups
    pub blocked: Vec<String>,
}

impl Default for WhatsAppConfig {
    fn default() -> Self {
        Self {
            phone_number: None,
            business_api_token: None,
            business_phone_id: None,
            webhook_verify_token: None,
            use_business_api: false,
            sidecar_url: None,
            session_path: None,
            auto_reply: false,
            allowed_contacts: vec![],
            blocked: vec![],
        }
    }
}

impl WhatsAppConfig {
    /// Is this using Business API?
    pub fn is_business(&self) -> bool {
        self.use_business_api && self.business_api_token.is_some()
    }
    
    /// Is sidecar configured?
    pub fn has_sidecar(&self) -> bool {
        self.sidecar_url.is_some()
    }
    
    /// Create for Business API
    pub fn business(token: &str, phone_id: &str) -> Self {
        Self {
            use_business_api: true,
            business_api_token: Some(token.to_string()),
            business_phone_id: Some(phone_id.to_string()),
            ..Self::default()
        }
    }
    
    /// Create with sidecar
    pub fn with_sidecar(url: &str, phone: &str) -> Self {
        Self {
            phone_number: Some(phone.to_string()),
            sidecar_url: Some(url.to_string()),
            ..Self::default()
        }
    }
}

/// WhatsApp channel provider
pub struct WhatsAppChannel {
    config: WhatsAppConfig,
    http_client: reqwest::Client,
}

impl WhatsAppChannel {
    pub fn new(config: WhatsAppConfig) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
        }
    }
    
    /// Send via Business API
    pub async fn send_business(&self, to: &str, text: &str) -> anyhow::Result<()> {
        let Some(ref token) = self.config.business_api_token else {
            return Err(anyhow::anyhow!("Business API token not configured"));
        };
        
        let Some(ref phone_id) = self.config.business_phone_id else {
            return Err(anyhow::anyhow!("Business phone ID not configured"));
        };
        
        let url = format!(
            "https://graph.facebook.com/v18.0/{}/messages",
            phone_id
        );
        
        let body = serde_json::json!({
            "messaging_product": "whatsapp",
            "recipient_type": "individual",
            "to": to,
            "type": "text",
            "text": {
                "preview_url": false,
                "body": text
            }
        });
        
        let response = self.http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(anyhow::anyhow!("WhatsApp API error: {}", error));
        }
        
        info!("WhatsApp message sent to {}", to);
        Ok(())
    }
    
    /// Send via sidecar (Baileys/whatsmeow)
    pub async fn send_sidecar(&self, to: &str, text: &str) -> anyhow::Result<()> {
        let Some(ref url) = self.config.sidecar_url else {
            return Err(anyhow::anyhow!("Sidecar URL not configured"));
        };
        
        let body = serde_json::json!({
            "to": to,
            "text": text
        });
        
        let response = self.http_client
            .post(&format!("{}/send", url))
            .json(&body)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(anyhow::anyhow!("Sidecar error: {}", error));
        }
        
        info!("WhatsApp message sent via sidecar to {}", to);
        Ok(())
    }
    
    /// Parse webhook payload
    pub fn parse_webhook(&self, payload: &serde_json::Value) -> Option<InboundMessage> {
        // Meta Business API webhook format
        let entry = payload.get("entry")?.as_array()?.first()?;
        let changes = entry.get("changes")?.as_array()?.first()?;
        let value = changes.get("value")?;
        let messages = value.get("messages")?.as_array()?.first()?;
        
        let from = messages.get("from")?.as_str()?;
        let text = messages.get("text")?.get("body")?.as_str()?;
        let id = messages.get("id")?.as_str()?;
        
        Some(InboundMessage {
            id: id.to_string(),
            channel: "whatsapp".to_string(),
            chat_id: from.to_string(),
            sender_id: from.to_string(),
            sender_name: None,
            text: Some(text.to_string()),
            reply_to: None,
            media: None,
            chat_type: ChatType::Direct,
            timestamp: chrono::Utc::now(),
        })
    }
}

/// Sidecar interface for Node.js/Baileys integration
#[async_trait]
pub trait WhatsAppSidecar: Send + Sync {
    /// Start the sidecar connection
    async fn start(&mut self) -> anyhow::Result<()>;
    
    /// Stop the sidecar
    async fn stop(&mut self) -> anyhow::Result<()>;
    
    /// Send a message
    async fn send(&self, to: &str, text: &str) -> anyhow::Result<String>;
    
    /// Send media
    async fn send_media(&self, to: &str, url: &str, caption: Option<&str>) -> anyhow::Result<String>;
    
    /// Mark as read
    async fn mark_read(&self, message_id: &str) -> anyhow::Result<()>;
    
    /// Get QR code for pairing
    async fn get_qr(&self) -> Option<String>;
    
    /// Check if connected
    async fn is_connected(&self) -> bool;
}

/// Stub sidecar implementation (placeholder)
pub struct StubSidecar;

#[async_trait]
impl WhatsAppSidecar for StubSidecar {
    async fn start(&mut self) -> anyhow::Result<()> {
        warn!("WhatsApp sidecar stub - start called");
        Ok(())
    }
    
    async fn stop(&mut self) -> anyhow::Result<()> {
        warn!("WhatsApp sidecar stub - stop called");
        Ok(())
    }
    
    async fn send(&self, _to: &str, _text: &str) -> anyhow::Result<String> {
        Err(anyhow::anyhow!("WhatsApp sidecar not implemented - use Baileys/whatsmeow"))
    }
    
    async fn send_media(&self, _to: &str, _url: &str, _caption: Option<&str>) -> anyhow::Result<String> {
        Err(anyhow::anyhow!("WhatsApp sidecar not implemented"))
    }
    
    async fn mark_read(&self, _message_id: &str) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("WhatsApp sidecar not implemented"))
    }
    
    async fn get_qr(&self) -> Option<String> {
        warn!("WhatsApp sidecar stub - QR not available");
        None
    }
    
    async fn is_connected(&self) -> bool {
        false
    }
}

/// Setup instructions for WhatsApp
pub fn setup_instructions() -> String {
    r#"
# WhatsApp Channel Setup

## Option 1: WhatsApp Business API (Recommended for Production)

1. Create Meta Business account
2. Add WhatsApp Business API integration
3. Get phone number ID and access token
4. Configure webhook endpoint

Config:
```json
{
  "whatsapp": {
    "use_business_api": true,
    "business_api_token": "YOUR_TOKEN",
    "business_phone_id": "+1234567890",
    "webhook_verify_token": "VERIFY_TOKEN"
  }
}
```

## Option 2: WhatsApp Personal (Baileys Sidecar)

Requires Node.js sidecar:

```bash
# Install Baileys sidecar
npm install @adiwajshing/baileys whatsapp-sidecar

# Run sidecar
node sidecar.js --port 3001
```

Config:
```json
{
  "whatsapp": {
    "sidecar_url": "http://localhost:3001",
    "phone_number": "+1234567890"
  }
}
```

## Option 3: Go Sidecar (whatsmeow)

```go
// Go implementation using whatsmeow
// See: github.com/harshm7n/whatsapp-sidecar
```

"#
    .to_string()
}