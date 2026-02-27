pub mod access_control;
pub mod channels;

use async_trait::async_trait;
use klaw_core::types::{InboundMessage, Media};
use tokio::sync::mpsc;

/// Every channel (Telegram, Discord, WebChat, etc.) implements this trait
#[async_trait]
pub trait Channel: Send + Sync {
    /// Channel name (e.g., "telegram", "discord", "webchat")
    fn name(&self) -> &str;

    /// Start receiving messages — sends inbound messages via the channel
    async fn start(&mut self, tx: mpsc::Sender<InboundMessage>) -> anyhow::Result<()>;

    /// Send a text message
    async fn send_text(&self, chat_id: &str, text: &str, reply_to: Option<&str>) -> anyhow::Result<()>;

    /// Send media
    async fn send_media(&self, chat_id: &str, media: &Media) -> anyhow::Result<()>;

    /// Send typing indicator
    async fn send_typing(&self, chat_id: &str) -> anyhow::Result<()>;

    /// Stop the channel
    async fn stop(&mut self) -> anyhow::Result<()>;
}

// Re-export all channels
pub use channels::*;

/// List all available channel names
pub fn available_channels() -> Vec<&'static str> {
    vec![
        "webchat",
        "telegram",
        "discord",
        "slack",
        "whatsapp",
        "signal",
        "irc",
        "googlechat",
        "bluebubbles",
        "imessage",
        // Plugins
        "feishu",
        "mattermost",
        "msteams",
        "synology-chat",
        "line",
        "nextcloud-talk",
        "matrix",
        "nostr",
        "tlon",
        "twitch",
        "zalo",
        "zalouser",
    ]
}
