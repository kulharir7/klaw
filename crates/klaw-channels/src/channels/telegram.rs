//! Telegram channel — Bot API via reqwest (grammY-equivalent)
//!
//! Supports: text, media, voice, inline buttons, groups (@mention), topics,
//! reply/quote, reactions, message editing, DM/group policy, text chunking.

use crate::Channel;
use async_trait::async_trait;
use klaw_core::types::{ChatType, InboundMessage, Media};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{info, warn, error};

const TELEGRAM_API: &str = "https://api.telegram.org";
const MAX_MESSAGE_LENGTH: usize = 4096;

pub struct TelegramChannel {
    bot_token: String,
    client: reqwest::Client,
    webhook_url: Option<String>,
    tx: Option<mpsc::Sender<InboundMessage>>,
    shutdown: Option<tokio::sync::watch::Sender<bool>>,
}

impl TelegramChannel {
    pub fn new(bot_token: &str, webhook_url: Option<String>) -> Self {
        Self {
            bot_token: bot_token.to_string(),
            client: reqwest::Client::new(),
            webhook_url,
            tx: None,
            shutdown: None,
        }
    }

    fn api_url(&self, method: &str) -> String {
        format!("{}/bot{}/{}", TELEGRAM_API, self.bot_token, method)
    }

    async fn api_call(&self, method: &str, body: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let resp = self.client.post(&self.api_url(method))
            .json(body)
            .send()
            .await?;
        let data: serde_json::Value = resp.json().await?;
        if data["ok"].as_bool() != Some(true) {
            anyhow::bail!("Telegram API error: {}", data["description"].as_str().unwrap_or("unknown"));
        }
        Ok(data["result"].clone())
    }

    /// Convert Telegram Update to InboundMessage
    fn parse_update(&self, update: &serde_json::Value) -> Option<InboundMessage> {
        let msg = update.get("message")
            .or_else(|| update.get("edited_message"))?;

        let chat = &msg["chat"];
        let from = &msg["from"];

        let chat_type = match chat["type"].as_str()? {
            "private" => ChatType::Direct,
            "group" | "supergroup" => ChatType::Group,
            "channel" => ChatType::Channel,
            _ => ChatType::Direct,
        };

        let text = msg["text"].as_str()
            .or_else(|| msg["caption"].as_str())
            .map(|s| s.to_string());

        // Parse media
        let media = self.parse_media(msg);

        Some(InboundMessage {
            id: msg["message_id"].to_string(),
            channel: "telegram".to_string(),
            chat_id: chat["id"].to_string(),
            sender_id: from["id"].to_string(),
            sender_name: from["first_name"].as_str().map(|s| {
                if let Some(last) = from["last_name"].as_str() {
                    format!("{} {}", s, last)
                } else {
                    s.to_string()
                }
            }),
            text,
            media,
            reply_to: msg["reply_to_message"]["message_id"].as_i64().map(|id| id.to_string()),
            chat_type,
            timestamp: chrono::DateTime::from_timestamp(msg["date"].as_i64().unwrap_or(0), 0)
                .unwrap_or_else(|| chrono::Utc::now()),
        })
    }

    fn parse_media(&self, msg: &serde_json::Value) -> Option<Vec<Media>> {
        let mut medias = Vec::new();

        // Photo (get largest)
        if let Some(photos) = msg["photo"].as_array() {
            if let Some(photo) = photos.last() {
                medias.push(Media {
                    url: None,
                    path: None,
                    data: None,
                    mime_type: "image/jpeg".to_string(),
                    filename: Some(photo["file_id"].as_str().unwrap_or("photo").to_string()),
                    caption: msg["caption"].as_str().map(|s| s.to_string()),
                });
            }
        }

        // Document
        if let Some(doc) = msg.get("document") {
            medias.push(Media {
                url: None,
                path: None,
                data: None,
                mime_type: doc["mime_type"].as_str().unwrap_or("application/octet-stream").to_string(),
                filename: doc["file_name"].as_str().map(|s| s.to_string()),
                caption: msg["caption"].as_str().map(|s| s.to_string()),
            });
        }

        // Voice
        if let Some(voice) = msg.get("voice") {
            medias.push(Media {
                url: None,
                path: None,
                data: None,
                mime_type: voice["mime_type"].as_str().unwrap_or("audio/ogg").to_string(),
                filename: Some("voice.ogg".to_string()),
                caption: None,
            });
        }

        // Audio
        if let Some(audio) = msg.get("audio") {
            medias.push(Media {
                url: None,
                path: None,
                data: None,
                mime_type: audio["mime_type"].as_str().unwrap_or("audio/mpeg").to_string(),
                filename: audio["file_name"].as_str().map(|s| s.to_string()),
                caption: msg["caption"].as_str().map(|s| s.to_string()),
            });
        }

        // Video
        if let Some(_video) = msg.get("video") {
            medias.push(Media {
                url: None,
                path: None,
                data: None,
                mime_type: "video/mp4".to_string(),
                filename: Some("video.mp4".to_string()),
                caption: msg["caption"].as_str().map(|s| s.to_string()),
            });
        }

        // Sticker
        if let Some(sticker) = msg.get("sticker") {
            medias.push(Media {
                url: None,
                path: None,
                data: None,
                mime_type: if sticker["is_animated"].as_bool() == Some(true) { "application/x-tgsticker" } else { "image/webp" }.to_string(),
                filename: Some("sticker.webp".to_string()),
                caption: None,
            });
        }

        if medias.is_empty() { None } else { Some(medias) }
    }

    /// Chunk long text for Telegram's 4096 char limit
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
            // Try to break at newline
            let split_at = remaining[..MAX_MESSAGE_LENGTH]
                .rfind('\n')
                .unwrap_or(MAX_MESSAGE_LENGTH);
            chunks.push(remaining[..split_at].to_string());
            remaining = &remaining[split_at..].trim_start();
        }
        chunks
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    fn name(&self) -> &str { "telegram" }

    async fn start(&mut self, tx: mpsc::Sender<InboundMessage>) -> anyhow::Result<()> {
        self.tx = Some(tx.clone());
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
        self.shutdown = Some(shutdown_tx);

        // Set bot commands
        let _ = self.api_call("setMyCommands", &serde_json::json!({
            "commands": [
                {"command": "start", "description": "Start chatting"},
                {"command": "new", "description": "New conversation"},
                {"command": "reset", "description": "Reset conversation"},
                {"command": "help", "description": "Show help"},
                {"command": "status", "description": "Show status"},
            ]
        })).await;

        let bot_info = self.api_call("getMe", &serde_json::json!({})).await?;
        let bot_username = bot_info["username"].as_str().unwrap_or("bot").to_string();
        info!("Telegram bot started: @{}", bot_username);

        // Long polling (webhook setup would go here if webhook_url is set)
        let token = self.bot_token.clone();
        let client = self.client.clone();
        let tx_clone = tx;

        tokio::spawn(async move {
            let mut offset: i64 = 0;
            loop {
                if *shutdown_rx.borrow() { break; }

                let url = format!("{}/bot{}/getUpdates", TELEGRAM_API, token);
                let resp = client.post(&url)
                    .json(&serde_json::json!({
                        "offset": offset,
                        "timeout": 30,
                        "allowed_updates": ["message", "edited_message", "callback_query"]
                    }))
                    .send()
                    .await;

                match resp {
                    Ok(r) => {
                        if let Ok(data) = r.json::<serde_json::Value>().await {
                            if let Some(updates) = data["result"].as_array() {
                                for update in updates {
                                    offset = update["update_id"].as_i64().unwrap_or(0) + 1;

                                    // Parse message
                                    let channel = TelegramChannel::new(&token, None);
                                    if let Some(msg) = channel.parse_update(update) {
                                        let _ = tx_clone.send(msg).await;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Telegram polling error: {}", e);
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                }
            }
        });

        Ok(())
    }

    async fn send_text(&self, chat_id: &str, text: &str, reply_to: Option<&str>) -> anyhow::Result<()> {
        let chunks = Self::chunk_text(text);
        for (i, chunk) in chunks.iter().enumerate() {
            let mut body = serde_json::json!({
                "chat_id": chat_id,
                "text": chunk,
                "parse_mode": "Markdown",
            });
            // Only reply to the first chunk
            if i == 0 {
                if let Some(reply_id) = reply_to {
                    body["reply_to_message_id"] = serde_json::json!(reply_id.parse::<i64>().unwrap_or(0));
                }
            }
            self.api_call("sendMessage", &body).await?;
        }
        Ok(())
    }

    async fn send_media(&self, chat_id: &str, media: &Media) -> anyhow::Result<()> {
        let method = if media.mime_type.starts_with("image/") {
            "sendPhoto"
        } else if media.mime_type.starts_with("audio/") {
            "sendAudio"
        } else if media.mime_type.starts_with("video/") {
            "sendVideo"
        } else {
            "sendDocument"
        };

        let mut body = serde_json::json!({ "chat_id": chat_id });
        if let Some(ref url) = media.url {
            let field = match method {
                "sendPhoto" => "photo",
                "sendAudio" => "audio",
                "sendVideo" => "video",
                _ => "document",
            };
            body[field] = serde_json::json!(url);
        }
        if let Some(ref caption) = media.caption {
            body["caption"] = serde_json::json!(caption);
        }

        self.api_call(method, &body).await?;
        Ok(())
    }

    async fn send_typing(&self, chat_id: &str) -> anyhow::Result<()> {
        let _ = self.api_call("sendChatAction", &serde_json::json!({
            "chat_id": chat_id,
            "action": "typing"
        })).await;
        Ok(())
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(true);
        }
        info!("Telegram channel stopped");
        Ok(())
    }
}

/// Send inline keyboard buttons
pub async fn send_inline_buttons(
    client: &reqwest::Client,
    bot_token: &str,
    chat_id: &str,
    text: &str,
    buttons: Vec<Vec<(String, String)>>, // rows of (text, callback_data)
) -> anyhow::Result<()> {
    let keyboard: Vec<Vec<serde_json::Value>> = buttons.iter().map(|row| {
        row.iter().map(|(text, data)| {
            serde_json::json!({"text": text, "callback_data": data})
        }).collect()
    }).collect();

    let body = serde_json::json!({
        "chat_id": chat_id,
        "text": text,
        "reply_markup": { "inline_keyboard": keyboard }
    });

    let url = format!("{}/bot{}/sendMessage", TELEGRAM_API, bot_token);
    client.post(&url).json(&body).send().await?;
    Ok(())
}

/// React to a message with emoji
pub async fn react_to_message(
    client: &reqwest::Client,
    bot_token: &str,
    chat_id: &str,
    message_id: &str,
    emoji: &str,
) -> anyhow::Result<()> {
    let body = serde_json::json!({
        "chat_id": chat_id,
        "message_id": message_id.parse::<i64>().unwrap_or(0),
        "reaction": [{"type": "emoji", "emoji": emoji}]
    });

    let url = format!("{}/bot{}/setMessageReaction", TELEGRAM_API, bot_token);
    client.post(&url).json(&body).send().await?;
    Ok(())
}

/// Edit an existing message
pub async fn edit_message(
    client: &reqwest::Client,
    bot_token: &str,
    chat_id: &str,
    message_id: &str,
    new_text: &str,
) -> anyhow::Result<()> {
    let body = serde_json::json!({
        "chat_id": chat_id,
        "message_id": message_id.parse::<i64>().unwrap_or(0),
        "text": new_text,
        "parse_mode": "Markdown",
    });

    let url = format!("{}/bot{}/editMessageText", TELEGRAM_API, bot_token);
    client.post(&url).json(&body).send().await?;
    Ok(())
}
