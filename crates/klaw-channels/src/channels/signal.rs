//! Signal Channel Placeholder
//! 
//! Full implementation requires signal-cli:
//! - Java: github.com/AsamK/signal-cli
//! - REST API wrapper: bbernhard/signal-cli-rest-api
//! 
//! This module provides the interface spec and stub implementations.

use async_trait::async_trait;
use klaw_core::types::{ChatType, InboundMessage, OutboundMessage};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// Signal configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SignalConfig {
    /// Phone number (E.164 format)
    pub phone_number: Option<String>,
    /// Signal-cli REST API URL
    pub api_url: Option<String>,
    /// Account UUID
    pub account_id: Option<String>,
    /// Device ID
    pub device_id: Option<i32>,
    /// CAPTCHA token (for registration)
    pub captcha_token: Option<String>,
    /// Use server-based signal-cli
    pub use_server: bool,
    /// Allow unknown contacts
    pub allow_unknown: bool,
    /// Auto-reply to messages
    pub auto_reply: bool,
    /// Whisper mode (private replies)
    pub whisper_mode: bool,
}

impl Default for SignalConfig {
    fn default() -> Self {
        Self {
            phone_number: None,
            api_url: None,
            account_id: None,
            device_id: None,
            captcha_token: None,
            use_server: false,
            allow_unknown: false,
            auto_reply: false,
            whisper_mode: false,
        }
    }
}

impl SignalConfig {
    /// Is signal-cli configured?
    pub fn is_configured(&self) -> bool {
        self.api_url.is_some() || self.use_server
    }
    
    /// Create with API URL
    pub fn with_api(url: &str, phone: &str) -> Self {
        Self {
            api_url: Some(url.to_string()),
            phone_number: Some(phone.to_string()),
            ..Self::default()
        }
    }
    
    /// Use embedded signal-cli server
    pub fn use_embedded() -> Self {
        Self {
            use_server: true,
            ..Self::default()
        }
    }
}

/// Signal channel provider
pub struct SignalChannel {
    config: SignalConfig,
    http_client: reqwest::Client,
}

impl SignalChannel {
    pub fn new(config: SignalConfig) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
        }
    }
    
    /// Send via signal-cli REST API
    pub async fn send(&self, to: &str, text: &str) -> anyhow::Result<()> {
        let Some(ref url) = self.config.api_url else {
            return Err(anyhow::anyhow!("Signal API URL not configured"));
        };
        
        let Some(ref phone) = self.config.phone_number else {
            return Err(anyhow::anyhow!("Phone number not configured"));
        };
        
        let body = serde_json::json!({
            "message": text,
            "number": phone,
            "recipients": [to]
        });
        
        let response = self.http_client
            .post(&format!("{}/v2/send", url))
            .json(&body)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(anyhow::anyhow!("Signal API error: {}", error));
        }
        
        info!("Signal message sent to {}", to);
        Ok(())
    }
    
    /// Send to group
    pub async fn send_group(&self, group_id: &str, text: &str) -> anyhow::Result<()> {
        let Some(ref url) = self.config.api_url else {
            return Err(anyhow::anyhow!("Signal API URL not configured"));
        };
        
        let Some(ref phone) = self.config.phone_number else {
            return Err(anyhow::anyhow!("Phone number not configured"));
        };
        
        let body = serde_json::json!({
            "message": text,
            "number": phone,
            "recipients": [group_id]
        });
        
        let response = self.http_client
            .post(&format!("{}/v2/groups/{}/send", url, group_id))
            .json(&body)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(anyhow::anyhow!("Signal API error: {}", error));
        }
        
        info!("Signal group message sent to {}", group_id);
        Ok(())
    }
    
    /// Parse webhook payload from signal-cli
    pub fn parse_webhook(&self, payload: &serde_json::Value) -> Option<InboundMessage> {
        let envelope = payload.get("envelope")?;
        
        let source = envelope.get("source")?.as_str()?;
        let source_device = envelope.get("sourceDevice")?.as_i64()? as i32;
        let timestamp = envelope.get("timestamp")?.as_i64()?;
        
        // Handle different message types
        let text = if let Some(dataMessage) = envelope.get("dataMessage") {
            dataMessage.get("message")?.as_str()?.to_string()
        } else if let Some(syncMessage) = envelope.get("syncMessage") {
            if let Some(sent) = syncMessage.get("sentMessage") {
                sent.get("message")?.as_str()?.to_string()
            } else {
                return None;
            }
        } else {
            return None;
        };
        
        Some(InboundMessage {
            id: timestamp.to_string(),
            channel: "signal".to_string(),
            chat_id: source.to_string(),
            sender_id: source.to_string(),
            sender_name: None,
            text: Some(text),
            reply_to: None,
            media: None,
            chat_type: ChatType::Direct,
            timestamp: chrono::DateTime::from_timestamp_millis(timestamp).unwrap_or_else(chrono::Utc::now),
        })
    }
    
    /// List groups
    pub async fn list_groups(&self) -> anyhow::Result<Vec<SignalGroup>> {
        let Some(ref url) = self.config.api_url else {
            return Err(anyhow::anyhow!("Signal API URL not configured"));
        };
        
        let Some(ref phone) = self.config.phone_number else {
            return Err(anyhow::anyhow!("Phone number not configured"));
        };
        
        let response = self.http_client
            .get(&format!("{}/v1/groups/{}", url, phone))
            .send()
            .await?;
        
        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(anyhow::anyhow!("Signal API error: {}", error));
        }
        
        let groups: Vec<SignalGroup> = response.json().await?;
        Ok(groups)
    }
}

/// Signal group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalGroup {
    pub id: String,
    pub name: String,
    pub members: Vec<String>,
    pub is_admin: bool,
}

/// Signal sidecar interface
#[async_trait]
pub trait SignalSidecar: Send + Sync {
    /// Register phone number
    async fn register(&mut self, phone: &str, captcha: Option<&str>) -> anyhow::Result<()>;
    
    /// Verify with SMS code
    async fn verify(&mut self, code: &str) -> anyhow::Result<String>;
    
    /// Start listening for messages
    async fn start(&mut self) -> anyhow::Result<()>;
    
    /// Stop listening
    async fn stop(&mut self) -> anyhow::Result<()>;
    
    /// Send message
    async fn send(&self, to: &str, text: &str) -> anyhow::Result<String>;
    
    /// Get linked devices
    async fn get_devices(&self) -> anyhow::Result<Vec<String>>;
    
    /// Link new device (returns QR URL)
    async fn link_device(&self) -> anyhow::Result<String>;
}

/// Stub sidecar implementation (placeholder)
pub struct StubSignalSidecar;

#[async_trait]
impl SignalSidecar for StubSignalSidecar {
    async fn register(&mut self, _phone: &str, _captcha: Option<&str>) -> anyhow::Result<()> {
        warn!("Signal sidecar stub - register called");
        Ok(())
    }
    
    async fn verify(&mut self, _code: &str) -> anyhow::Result<String> {
        Err(anyhow::anyhow!("Signal sidecar not implemented"))
    }
    
    async fn start(&mut self) -> anyhow::Result<()> {
        warn!("Signal sidecar stub - start called");
        Ok(())
    }
    
    async fn stop(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
    
    async fn send(&self, _to: &str, _text: &str) -> anyhow::Result<String> {
        Err(anyhow::anyhow!("Signal sidecar not implemented - use signal-cli"))
    }
    
    async fn get_devices(&self) -> anyhow::Result<Vec<String>> {
        Ok(vec![])
    }
    
    async fn link_device(&self) -> anyhow::Result<String> {
        Err(anyhow::anyhow!("Signal sidecar not implemented"))
    }
}

/// Setup instructions for Signal
pub fn setup_instructions() -> String {
    r#"
# Signal Channel Setup

## Option 1: signal-cli REST API (Recommended)

1. Install signal-cli:
```bash
# Debian/Ubuntu
sudo apt install signal-cli

# Arch Linux
yay -S signal-cli

# Using Docker
docker run -v $HOME/.signal-cli:/signal/bin/ bbernhard/signal-cli-rest-api
```

2. Register phone number:
```bash
signal-cli -u +1234567890 register
signal-cli -u +1234567890 verify 123456
```

3. Start REST API:
```bash
signal-cli-rest-api -p 8080
```

4. Configure Klaw:
```json
{
  "signal": {
    "phone_number": "+1234567890",
    "api_url": "http://localhost:8080"
  }
}
```

## Option 2: Docker Compose

```yaml
version: '3'
services:
  signal-cli:
    image: bbernhard/signal-cli-rest-api
    ports:
      - "8080:8080"
    volumes:
      - ./signal:/root/.local/share/signal-cli
    environment:
      - MODE=normal
```

## Option 3: Native Java signal-cli

```bash
# Build from source
git clone https://github.com/AsamK/signal-cli
cd signal-cli
./gradlew build

# Run
java -jar build/libs/signal-cli.jar -u +1234567890 daemon
```

## Notes

- Signal requires phone verification
- CAPTCHA may be needed for registration
- Groups require admin status to add bots
- Rate limits: ~60 messages/hour per recipient

"#
    .to_string()
}