//! WhatsApp channel — via Baileys (JS sidecar) or whatsmeow (Go sidecar) or Business API
//! Requires QR pairing. Stores session state on disk.
use crate::Channel;
use async_trait::async_trait;
use klaw_core::types::{InboundMessage, Media};
use tokio::sync::mpsc;
use tracing::info;

pub struct WhatsAppChannel { tx: Option<mpsc::Sender<InboundMessage>> }
impl WhatsAppChannel { pub fn new() -> Self { Self { tx: None } } }

#[async_trait]
impl Channel for WhatsAppChannel {
    fn name(&self) -> &str { "whatsapp" }
    async fn start(&mut self, tx: mpsc::Sender<InboundMessage>) -> anyhow::Result<()> { self.tx = Some(tx); info!("WhatsApp channel started (Baileys sidecar TODO)"); Ok(()) }
    async fn send_text(&self, _chat_id: &str, _text: &str, _reply_to: Option<&str>) -> anyhow::Result<()> { Ok(()) }
    async fn send_media(&self, _chat_id: &str, _media: &Media) -> anyhow::Result<()> { Ok(()) }
    async fn send_typing(&self, _chat_id: &str) -> anyhow::Result<()> { Ok(()) }
    async fn stop(&mut self) -> anyhow::Result<()> { info!("WhatsApp stopped"); Ok(()) }
}
