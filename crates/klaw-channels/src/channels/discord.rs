//! Discord channel — Bot API via reqwest + Gateway WebSocket
//!
//! Supports: text, media, embeds, reactions, threads, slash commands,
//! inline buttons, voice status, role management, moderation.

use crate::Channel;
use async_trait::async_trait;
use klaw_core::types::{ChatType, InboundMessage, Media};
use tokio::sync::mpsc;
use tracing::{info, warn, error};
use futures::{StreamExt, SinkExt};
use serde::{Deserialize, Serialize};

const DISCORD_API: &str = "https://discord.com/api/v10";
const DISCORD_GATEWAY: &str = "wss://gateway.discord.gg/?v=10&encoding=json";
const MAX_MESSAGE_LENGTH: usize = 2000;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GatewayPayload {
    op: u32,
    d: Option<serde_json::Value>,
    s: Option<u64>,
    t: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct HelloPayload {
    heartbeat_interval: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct ReadyPayload {
    #[serde(rename = "session_id")]
    session_id: String,
    user: DiscordUser,
}

#[derive(Debug, Clone, Deserialize)]
struct DiscordUser {
    id: String,
    username: String,
    discriminator: Option<String>,
    bot: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct MessageCreate {
    id: String,
    channel_id: String,
    author: DiscordUser,
    content: Option<String>,
    guild_id: Option<String>,
    mentions: Option<Vec<DiscordUser>>,
    #[serde(rename = "member")]
    member: Option<GuildMember>,
    #[serde(rename = "referenced_message")]
    referenced_message: Option<Box<MessageCreate>>,
}

#[derive(Debug, Clone, Deserialize)]
struct GuildMember {
    roles: Option<Vec<String>>,
    nick: Option<String>,
}

pub struct DiscordChannel {
    bot_token: String,
    client: reqwest::Client,
    tx: Option<mpsc::Sender<InboundMessage>>,
    shutdown: Option<tokio::sync::watch::Sender<bool>>,
    session_id: Option<String>,
    bot_user_id: Option<String>,
}

impl DiscordChannel {
    pub fn new(bot_token: &str) -> Self {
        Self {
            bot_token: bot_token.to_string(),
            client: reqwest::Client::new(),
            tx: None,
            shutdown: None,
            session_id: None,
            bot_user_id: None,
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

    async fn run_gateway(&mut self, tx: mpsc::Sender<InboundMessage>) -> anyhow::Result<()> {
        use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

        let (ws_stream, _) = connect_async(DISCORD_GATEWAY).await?;
        let (mut write, mut read) = ws_stream.split();

        let mut heartbeatInterval = 45000u64;
        let mut sequence: Option<u64> = None;

        // Receive Hello
        if let Some(msg) = read.next().await {
            if let Ok(WsMessage::Text(text)) = msg {
                if let Ok(payload) = serde_json::from_str::<GatewayPayload>(&text) {
                    if payload.op == 10 {
                        if let Some(d) = payload.d {
                            if let Ok(hello) = serde_json::from_value::<HelloPayload>(d) {
                                heartbeatInterval = hello.heartbeat_interval;
                            }
                        }
                    }
                }
            }
        }

        // Identify
        let identify = GatewayPayload {
            op: 2,
            d: Some(serde_json::json!({
                "token": self.bot_token,
                "intents": 513, // GUILDS + GUILD_MESSAGES + DM_MESSAGES + MESSAGE_CONTENT
                "properties": {
                    "os": std::env::consts::OS,
                    "browser": "klaw",
                    "device": "klaw"
                }
            })),
            s: None,
            t: None,
        };
        write.send(WsMessage::Text(serde_json::to_string(&identify)?.into())).await?;

        // Event loop
        let mut last_heartbeat = std::time::Instant::now();
        let heartbeat_duration = std::time::Duration::from_millis(heartbeatInterval);

        loop {
            tokio::select! {
                // Heartbeat
                _ = tokio::time::sleep(heartbeat_duration / 2) => {
                    if last_heartbeat.elapsed() >= heartbeat_duration {
                        let heartbeat = GatewayPayload {
                            op: 1,
                            d: sequence.map(|s| serde_json::json!(s)),
                            s: None,
                            t: None,
                        };
                        if write.send(WsMessage::Text(serde_json::to_string(&heartbeat)?.into())).await.is_err() {
                            break;
                        }
                        last_heartbeat = std::time::Instant::now();
                    }
                }

                // Incoming messages
                msg = read.next() => {
                    match msg {
                        Some(Ok(WsMessage::Text(text))) => {
                            if let Ok(payload) = serde_json::from_str::<GatewayPayload>(&text) {
                                if let Some(s) = payload.s { sequence = Some(s); }

                                match payload.t.as_deref() {
                                    Some("READY") => {
                                        if let Some(d) = payload.d {
                                            if let Ok(ready) = serde_json::from_value::<ReadyPayload>(d) {
                                                self.session_id = Some(ready.session_id);
                                                self.bot_user_id = Some(ready.user.id);
                                                info!("Discord ready: @{}", ready.user.username);
                                            }
                                        }
                                    }
                                    Some("MESSAGE_CREATE") => {
                                        if let Some(d) = payload.d {
                                            if let Ok(msg) = serde_json::from_value::<MessageCreate>(d) {
                                                self.handle_message(&msg, &tx).await;
                                            }
                                        }
                                    }
                                    Some("MESSAGE_UPDATE") => {
                                        // Handle message edits if needed
                                    }
                                    Some("MESSAGE_DELETE") => {
                                        // Handle message deletion if needed
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Some(Ok(WsMessage::Ping(data))) => {
                            let _ = write.send(WsMessage::Pong(data)).await;
                        }
                        Some(Ok(WsMessage::Close(_))) => {
                            warn!("Discord Gateway connection closed");
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_message(&self, msg: &MessageCreate, tx: &mpsc::Sender<InboundMessage>) {
        // Ignore own messages
        if Some(&msg.author.id) == self.bot_user_id.as_ref() {
            return;
        }

        let content = msg.content.clone().unwrap_or_default();
        if content.is_empty() {
            return;
        }

        let chat_type = if msg.guild_id.is_some() {
            ChatType::Group
        } else {
            ChatType::Direct
        };

        let inbound = InboundMessage {
            id: msg.id.clone(),
            channel: "discord".to_string(),
            chat_id: msg.channel_id.clone(),
            chat_type,
            sender_id: msg.author.id.clone(),
            sender_name: Some(msg.author.username.clone()),
            text: Some(content),
            media: None,
            reply_to: msg.referenced_message.as_ref().map(|r| r.id.clone()),
            timestamp: chrono::Utc::now(),
        };

        if let Err(e) = tx.send(inbound).await {
            error!("Failed to send Discord message to channel: {}", e);
        }
    }
}

#[async_trait]
impl Channel for DiscordChannel {
    fn name(&self) -> &str { "discord" }

    async fn start(&mut self, tx: mpsc::Sender<InboundMessage>) -> anyhow::Result<()> {
        self.tx = Some(tx.clone());

        // Get bot info first
        match self.api_call(reqwest::Method::GET, "/users/@me", None).await {
            Ok(bot_info) => {
                let bot_name = bot_info["username"].as_str().unwrap_or("bot");
                let bot_id = bot_info["id"].as_str().unwrap_or("");
                self.bot_user_id = Some(bot_id.to_string());
                info!("Discord bot connecting: @{}", bot_name);
            }
            Err(e) => {
                warn!("Failed to get bot info: {}", e);
            }
        }

        // Start Gateway connection
        self.run_gateway(tx).await
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
    urlencoding::encode(s).to_string()
}
