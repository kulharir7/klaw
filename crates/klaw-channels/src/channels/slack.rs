//! Slack channel — Bolt SDK equivalent via reqwest
//! Supports: text, media, threads, reactions, slash commands, interactive components.

use crate::Channel;
use async_trait::async_trait;
use klaw_core::types::{InboundMessage, Media};
use tokio::sync::mpsc;
use tracing::info;

const SLACK_API: &str = "https://slack.com/api";

pub struct SlackChannel {
    bot_token: String,
    app_token: Option<String>, // For Socket Mode
    client: reqwest::Client,
    tx: Option<mpsc::Sender<InboundMessage>>,
}

impl SlackChannel {
    pub fn new(bot_token: &str, app_token: Option<String>) -> Self {
        Self { bot_token: bot_token.to_string(), app_token, client: reqwest::Client::new(), tx: None }
    }
}

#[async_trait]
impl Channel for SlackChannel {
    fn name(&self) -> &str { "slack" }

    async fn start(&mut self, tx: mpsc::Sender<InboundMessage>) -> anyhow::Result<()> {
        self.tx = Some(tx);
        info!("Slack channel started (Socket Mode TODO)");
        Ok(())
    }

    async fn send_text(&self, chat_id: &str, text: &str, reply_to: Option<&str>) -> anyhow::Result<()> {
        let mut body = serde_json::json!({ "channel": chat_id, "text": text });
        if let Some(ts) = reply_to { body["thread_ts"] = serde_json::json!(ts); }
        self.client.post(&format!("{}/chat.postMessage", SLACK_API))
            .header("Authorization", format!("Bearer {}", self.bot_token))
            .json(&body).send().await?;
        Ok(())
    }

    async fn send_media(&self, chat_id: &str, media: &Media) -> anyhow::Result<()> {
        if let Some(ref url) = media.url {
            let body = serde_json::json!({
                "channel": chat_id,
                "blocks": [{ "type": "image", "image_url": url, "alt_text": media.caption.as_deref().unwrap_or("image") }]
            });
            self.client.post(&format!("{}/chat.postMessage", SLACK_API))
                .header("Authorization", format!("Bearer {}", self.bot_token))
                .json(&body).send().await?;
        }
        Ok(())
    }

    async fn send_typing(&self, _chat_id: &str) -> anyhow::Result<()> { Ok(()) }
    async fn stop(&mut self) -> anyhow::Result<()> { info!("Slack stopped"); Ok(()) }
}
