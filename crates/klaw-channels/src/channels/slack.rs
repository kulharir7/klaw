//! Slack channel — Bolt SDK equivalent via reqwest + Socket Mode WebSocket
//! Supports: text, media, threads, reactions, slash commands, interactive components.

use crate::Channel;
use async_trait::async_trait;
use klaw_core::types::{ChatType, InboundMessage, Media};
use tokio::sync::mpsc;
use tracing::{info, warn, error};
use futures::{StreamExt, SinkExt};
use serde::{Deserialize, Serialize};

const SLACK_API: &str = "https://slack.com/api";

#[derive(Debug, Clone, Deserialize)]
struct SlackMessage {
    ts: String,
    channel: Option<String>,
    user: Option<String>,
    text: Option<String>,
    thread_ts: Option<String>,
    bot_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct SlackEvent {
    #[serde(rename = "type")]
    event_type: Option<String>,
    channel: Option<String>,
    user: Option<String>,
    text: Option<String>,
    ts: Option<String>,
    thread_ts: Option<String>,
    bot_id: Option<String>,
    message: Option<SlackMessage>,
}

#[derive(Debug, Clone, Deserialize)]
struct SlackPayload {
    #[serde(rename = "type")]
    payload_type: Option<String>,
    envelope_id: Option<String>,
    payload: Option<serde_json::Value>,
    event: Option<SlackEvent>,
}

#[derive(Debug, Clone, Serialize)]
struct SlackAck {
    envelope_id: String,
}

pub struct SlackChannel {
    bot_token: String,
    app_token: Option<String>,
    client: reqwest::Client,
    tx: Option<mpsc::Sender<InboundMessage>>,
    bot_user_id: Option<String>,
}

impl SlackChannel {
    pub fn new(bot_token: &str, app_token: Option<String>) -> Self {
        Self {
            bot_token: bot_token.to_string(),
            app_token,
            client: reqwest::Client::new(),
            tx: None,
            bot_user_id: None,
        }
    }

    async fn api_call(&self, method: reqwest::Method, path: &str, body: Option<&serde_json::Value>) -> anyhow::Result<serde_json::Value> {
        let mut req = self.client.request(method, &format!("{}{}", SLACK_API, path))
            .header("Authorization", format!("Bearer {}", self.bot_token))
            .header("Content-Type", "application/json");

        if let Some(b) = body {
            req = req.json(b);
        }

        let resp = req.send().await?;
        let result: serde_json::Value = resp.json().await?;

        if result["ok"].as_bool() != Some(true) {
            let error = result["error"].as_str().unwrap_or("unknown error");
            anyhow::bail!("Slack API error: {}", error);
        }

        Ok(result)
    }

    async fn run_socket_mode(&mut self, tx: mpsc::Sender<InboundMessage>) -> anyhow::Result<()> {
        use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

        let app_token = match &self.app_token {
            Some(t) => t,
            None => {
                warn!("Slack Socket Mode requires app_token. Provide SLACK_APP_TOKEN.");
                return Ok(());
            }
        };

        // Get WebSocket URL from Slack
        let resp = self.client.post("https://slack.com/api/apps.connections.open")
            .header("Authorization", format!("Bearer {}", app_token))
            .send()
            .await?;

        let result: serde_json::Value = resp.json().await?;
        let ws_url = match result["url"].as_str() {
            Some(url) => url.to_string(),
            None => {
                warn!("Failed to get Slack Socket Mode URL");
                return Ok(());
            }
        };

        info!("Slack Socket Mode connecting to: {}", ws_url);

        let (ws_stream, _) = connect_async(&ws_url).await?;
        let (mut write, mut read) = ws_stream.split();

        info!("Slack Socket Mode connected!");

        // Event loop
        loop {
            tokio::select! {
                msg = read.next() => {
                    match msg {
                        Some(Ok(WsMessage::Text(text))) => {
                            if let Ok(payload) = serde_json::from_str::<SlackPayload>(&text) {
                                // Acknowledge the message
                                if let Some(envelope_id) = &payload.envelope_id {
                                    let ack = SlackAck { envelope_id: envelope_id.clone() };
                                    if let Ok(ack_json) = serde_json::to_string(&ack) {
                                        let _ = write.send(WsMessage::Text(ack_json.into())).await;
                                    }
                                }

                                // Handle events
                                if let Some(event) = &payload.event {
                                    self.handle_event(event, &tx).await;
                                }
                            }
                        }
                        Some(Ok(WsMessage::Ping(data))) => {
                            let _ = write.send(WsMessage::Pong(data)).await;
                        }
                        Some(Ok(WsMessage::Close(_))) => {
                            warn!("Slack Socket Mode connection closed");
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_event(&self, event: &SlackEvent, tx: &mpsc::Sender<InboundMessage>) {
        // Only handle message events
        if event.event_type.as_deref() != Some("message") {
            return;
        }

        // Ignore bot messages
        if event.bot_id.is_some() {
            return;
        }

        let user_id = match (&event.user, &event.message) {
            (Some(u), _) => u.clone(),
            (None, Some(msg)) => msg.user.clone().unwrap_or_default(),
            (None, None) => return,
        };

        // Ignore own messages
        if Some(&user_id) == self.bot_user_id.as_ref() {
            return;
        }

        let text = match (&event.text, &event.message) {
            (Some(t), _) => t.clone(),
            (None, Some(msg)) => msg.text.clone().unwrap_or_default(),
            (None, None) => return,
        };

        if text.is_empty() {
            return;
        }

        let channel_id = event.channel.clone().unwrap_or_default();
        let ts = event.ts.clone().unwrap_or_default();
        let thread_ts = event.thread_ts.clone().or_else(|| event.message.as_ref().and_then(|m| m.thread_ts.clone()));

        let inbound = InboundMessage {
            id: ts.clone(),
            channel: "slack".to_string(),
            chat_id: channel_id,
            chat_type: ChatType::Group, // Slack messages are typically in channels
            sender_id: user_id,
            sender_name: None,
            text: Some(text),
            media: None,
            reply_to: thread_ts,
            timestamp: chrono::Utc::now(),
        };

        if let Err(e) = tx.send(inbound).await {
            error!("Failed to send Slack message to channel: {}", e);
        }
    }
}

#[async_trait]
impl Channel for SlackChannel {
    fn name(&self) -> &str { "slack" }

    async fn start(&mut self, tx: mpsc::Sender<InboundMessage>) -> anyhow::Result<()> {
        self.tx = Some(tx.clone());

        // Get bot info
        match self.api_call(reqwest::Method::GET, "/auth.test", None).await {
            Ok(info) => {
                if let Some(user_id) = info["user_id"].as_str() {
                    self.bot_user_id = Some(user_id.to_string());
                    info!("Slack bot connected: @{}", info["user"].as_str().unwrap_or("bot"));
                }
            }
            Err(e) => {
                warn!("Slack auth test failed: {}", e);
            }
        }

        // Start Socket Mode if app_token is available
        if self.app_token.is_some() {
            self.run_socket_mode(tx).await
        } else {
            info!("Slack channel started (no Socket Mode - provide SLACK_APP_TOKEN for real-time events)");
            Ok(())
        }
    }

    async fn send_text(&self, chat_id: &str, text: &str, reply_to: Option<&str>) -> anyhow::Result<()> {
        let mut body = serde_json::json!({ "channel": chat_id, "text": text });
        if let Some(ts) = reply_to {
            body["thread_ts"] = serde_json::json!(ts);
        }
        self.api_call(reqwest::Method::POST, "/chat.postMessage", Some(&body)).await?;
        Ok(())
    }

    async fn send_media(&self, chat_id: &str, media: &Media) -> anyhow::Result<()> {
        if let Some(ref url) = media.url {
            let body = serde_json::json!({
                "channel": chat_id,
                "blocks": [{
                    "type": "image",
                    "image_url": url,
                    "alt_text": media.caption.as_deref().unwrap_or("image")
                }]
            });
            self.api_call(reqwest::Method::POST, "/chat.postMessage", Some(&body)).await?;
        }
        Ok(())
    }

    async fn send_typing(&self, _chat_id: &str) -> anyhow::Result<()> {
        // Slack doesn't have a typing indicator API
        Ok(())
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        info!("Slack channel stopped");
        Ok(())
    }
}

/// Add reaction to message
pub async fn add_reaction(client: &reqwest::Client, token: &str, channel: &str, timestamp: &str, emoji: &str) -> anyhow::Result<()> {
    let body = serde_json::json!({ "channel": channel, "timestamp": timestamp, "name": emoji });
    client.post(&format!("{}/reactions.add", SLACK_API))
        .header("Authorization", format!("Bearer {}", token))
        .json(&body)
        .send().await?;
    Ok(())
}

/// Reply in thread
pub async fn reply_in_thread(client: &reqwest::Client, token: &str, channel: &str, thread_ts: &str, text: &str) -> anyhow::Result<serde_json::Value> {
    let body = serde_json::json!({ "channel": channel, "text": text, "thread_ts": thread_ts });
    let resp = client.post(&format!("{}/chat.postMessage", SLACK_API))
        .header("Authorization", format!("Bearer {}", token))
        .json(&body)
        .send().await?;
    Ok(resp.json().await?)
}
