//! Plugin channels — installed separately, all implement Channel trait
//! Each is a minimal stub ready for full implementation.

use crate::Channel;
use async_trait::async_trait;
use klaw_core::types::{InboundMessage, Media};
use tokio::sync::mpsc;
use tracing::info;

macro_rules! stub_channel {
    ($name:ident, $str_name:expr, $desc:expr) => {
        #[doc = $desc]
        pub struct $name { tx: Option<mpsc::Sender<InboundMessage>> }
        impl $name { pub fn new() -> Self { Self { tx: None } } }

        #[async_trait]
        impl Channel for $name {
            fn name(&self) -> &str { $str_name }
            async fn start(&mut self, tx: mpsc::Sender<InboundMessage>) -> anyhow::Result<()> {
                self.tx = Some(tx);
                info!("{} channel started (plugin TODO)", $str_name);
                Ok(())
            }
            async fn send_text(&self, _chat_id: &str, _text: &str, _reply_to: Option<&str>) -> anyhow::Result<()> { Ok(()) }
            async fn send_media(&self, _chat_id: &str, _media: &Media) -> anyhow::Result<()> { Ok(()) }
            async fn send_typing(&self, _chat_id: &str) -> anyhow::Result<()> { Ok(()) }
            async fn stop(&mut self) -> anyhow::Result<()> { info!("{} stopped", $str_name); Ok(()) }
        }
    };
}

stub_channel!(FeishuChannel, "feishu", "Feishu/Lark bot via WebSocket");
stub_channel!(MattermostChannel, "mattermost", "Mattermost Bot API + WebSocket");
stub_channel!(MSTeamsChannel, "msteams", "Microsoft Teams Bot Framework");
stub_channel!(SynologyChatChannel, "synology-chat", "Synology NAS Chat via webhooks");
stub_channel!(LineChannel, "line", "LINE Messaging API bot");
stub_channel!(NextcloudTalkChannel, "nextcloud-talk", "Nextcloud Talk self-hosted chat");
stub_channel!(MatrixChannel, "matrix", "Matrix protocol");
stub_channel!(NostrChannel, "nostr", "Decentralized DMs via NIP-04");
stub_channel!(TlonChannel, "tlon", "Urbit-based Tlon messenger");
stub_channel!(TwitchChannel, "twitch", "Twitch chat via IRC");
stub_channel!(ZaloChannel, "zalo", "Zalo Bot API");
stub_channel!(ZaloUserChannel, "zalouser", "Zalo personal account via QR login");
