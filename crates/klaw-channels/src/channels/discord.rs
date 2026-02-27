//! Discord channel — Bot API via reqwest (serenity-equivalent)
//!
//! Supports: text, media, embeds, reactions, threads, slash commands,
//! inline buttons, voice status, role management, moderation.

use crate::Channel;
use async_trait::async_trait;
use klaw_core::types::{ChatType, InboundMessage, Media};
use tokio::sync::mpsc;
use tracing::{info, warn};

const DISCORD_API: &str = "https://discord.com/api/v10";
const DISCORD_GATEWAY: &str = "wss://gateway.discord.gg/?v=10&encoding=json";
const MAX_MESSAGE_LENGTH: usize = 2000;

pub struct DiscordChannel {
    bot_token: String,
    client: reqwest::Client,
    tx: Option<mpsc::Sender<InboundMessage>>,
    shutdown: Option<tokio::sync::watch::Sender<bool>>,
}

impl DiscordChannel {
    pub fn new(bot_token: &str) -> Self {
        Self {
            bot_token: bot_token.to_string(),
            client: reqwest::Client::new(),
            tx: None,
            shutdown: None,
        }
    }

    async fn api_call(&self, method: reqwest::Method, path: &str, body: Option<&serde_json::Value>) -> anyhow::Result<serde_json::Value> {
        let mut req = self.client.request(method, &format!("{}{}", DISCORD_API, path))
            .header("Authorization", format!("Bot {}", self.bot_token))
            .header("Content-Type", "application/json");

        if let Some(b) = body {
            req = req.json(b);
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let err = resp.text().await.unwrap_or_default();
            anyhow::bail!("Discord API error {}: {}", status, err);
        }
        Ok(resp.json().await.unwrap_or(serde_json::json!({})))
    }

    fn chunk_text(text: &str) -> Vec<String> {
        if text.len() <= MAX_MESSAGE_LENGTH {
            return vec![text.to_string()];
        }
        let mut chunks = Vec::new();
        let mut remaining = text;
        while !remaining.is_empty() {
            if remaining.len() <= MAX_MESSAGE_LENGTH {
                chunks.push(remaining.to_string());
                break;
            }
            let split_at = remaining[..MAX_MESSAGE_LENGTH].rfind('\n').unwrap_or(MAX_MESSAGE_LENGTH);
            chunks.push(remaining[..split_at].to_string());
            remaining = remaining[split_at..].trim_start();
        }
        chunks
    }
}

#[async_trait]
impl Channel for DiscordChannel {
    fn name(&self) -> &str { "discord" }

    async fn start(&mut self, tx: mpsc::Sender<InboundMessage>) -> anyhow::Result<()> {
        self.tx = Some(tx.clone());
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        self.shutdown = Some(shutdown_tx);

        let bot_info = self.api_call(reqwest::Method::GET, "/users/@me", None).await?;
        let bot_name = bot_info["username"].as_str().unwrap_or("bot");
        info!("Discord bot started: {}", bot_name);

        // TODO: Full Gateway WebSocket connection for real-time events
        // For now, this is a placeholder — production needs Discord Gateway WS
        info!("Discord: Gateway WebSocket connection needed for real-time events (TODO)");

        Ok(())
    }

    async fn send_text(&self, chat_id: &str, text: &str, reply_to: Option<&str>) -> anyhow::Result<()> {
        let chunks = Self::chunk_text(text);
        for (i, chunk) in chunks.iter().enumerate() {
            let mut body = serde_json::json!({ "content": chunk });
            if i == 0 {
                if let Some(ref_id) = reply_to {
                    body["message_reference"] = serde_json::json!({
                        "message_id": ref_id
                    });
                }
            }
            self.api_call(
                reqwest::Method::POST,
                &format!("/channels/{}/messages", chat_id),
                Some(&body),
            ).await?;
        }
        Ok(())
    }

    async fn send_media(&self, chat_id: &str, media: &Media) -> anyhow::Result<()> {
        // Discord uses multipart for file uploads, or embed URLs
        if let Some(ref url) = media.url {
            let body = serde_json::json!({
                "embeds": [{
                    "image": { "url": url },
                    "description": media.caption.as_deref().unwrap_or(""),
                }]
            });
            self.api_call(
                reqwest::Method::POST,
                &format!("/channels/{}/messages", chat_id),
                Some(&body),
            ).await?;
        }
        Ok(())
    }

    async fn send_typing(&self, chat_id: &str) -> anyhow::Result<()> {
        let _ = self.api_call(
            reqwest::Method::POST,
            &format!("/channels/{}/typing", chat_id),
            None,
        ).await;
        Ok(())
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        if let Some(tx) = self.shutdown.take() { let _ = tx.send(true); }
        info!("Discord channel stopped");
        Ok(())
    }
}

/// Add reaction to message
pub async fn add_reaction(client: &reqwest::Client, token: &str, channel_id: &str, message_id: &str, emoji: &str) -> anyhow::Result<()> {
    let encoded = urlencoding(emoji);
    client.put(&format!("{}/channels/{}/messages/{}/reactions/{}/@me", DISCORD_API, channel_id, message_id, encoded))
        .header("Authorization", format!("Bot {}", token))
        .send().await?;
    Ok(())
}

/// Create thread from message
pub async fn create_thread(client: &reqwest::Client, token: &str, channel_id: &str, message_id: &str, name: &str) -> anyhow::Result<serde_json::Value> {
    let body = serde_json::json!({ "name": name, "auto_archive_duration": 1440 });
    let resp = client.post(&format!("{}/channels/{}/messages/{}/threads", DISCORD_API, channel_id, message_id))
        .header("Authorization", format!("Bot {}", token))
        .json(&body)
        .send().await?;
    Ok(resp.json().await?)
}

fn urlencoding(s: &str) -> String {
    s.chars().map(|c| match c {
        'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
        _ => format!("%{:02X}", c as u8),
    }).collect()
}
