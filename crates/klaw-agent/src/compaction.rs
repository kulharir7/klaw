use crate::provider::{ChatRequest, LlmProvider};
use klaw_core::session::Session;
use klaw_core::types::Message;
use tracing::info;

/// Configuration for session compaction
#[derive(Debug, Clone)]
pub struct CompactionConfig {
    pub mode: String,
    pub reserve_tokens_floor: u64,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            mode: "default".to_string(),
            reserve_tokens_floor: 24000,
        }
    }
}

/// Result of compaction
#[derive(Debug)]
pub struct CompactionResult {
    pub compacted: bool,
    pub messages_removed: usize,
    pub summary_tokens: u64,
}

/// Rough token estimate: 4 chars ≈ 1 token
fn estimate_tokens(messages: &[Message]) -> u64 {
    messages.iter().map(|m| m.content.len() as u64 / 4).sum()
}

/// Summarize old messages when session gets too long
pub async fn compact_session(
    session: &mut Session,
    provider: &dyn LlmProvider,
    model: &str,
    config: &CompactionConfig,
    max_context_tokens: u64,
) -> anyhow::Result<CompactionResult> {
    if config.mode == "off" {
        return Ok(CompactionResult {
            compacted: false,
            messages_removed: 0,
            summary_tokens: 0,
        });
    }

    let total_tokens = estimate_tokens(&session.messages);
    let threshold = max_context_tokens.saturating_sub(config.reserve_tokens_floor);

    if total_tokens <= threshold {
        return Ok(CompactionResult {
            compacted: false,
            messages_removed: 0,
            summary_tokens: 0,
        });
    }

    info!(
        "Compacting session: {} estimated tokens > {} threshold",
        total_tokens, threshold
    );

    let msg_count = session.messages.len();
    let compact_count = (msg_count as f64 * 0.7) as usize;
    if compact_count == 0 {
        return Ok(CompactionResult {
            compacted: false,
            messages_removed: 0,
            summary_tokens: 0,
        });
    }

    // Build summary from oldest 70%
    let old_messages: Vec<Message> = session.messages.drain(..compact_count).collect();
    let conversation_text: String = old_messages
        .iter()
        .map(|m| format!("{:?}: {}", m.role, m.content))
        .collect::<Vec<_>>()
        .join("\n");

    let summary_request = ChatRequest {
        model: model.to_string(),
        messages: vec![
            Message::system("Summarize this conversation concisely, preserving key facts, decisions, and context needed for continuation."),
            Message::user(&conversation_text),
        ],
        tools: None,
        temperature: Some(0.3),
        max_tokens: Some(2048),
        stream: false,
        thinking: None,
    };

    let response = provider.chat(summary_request).await?;
    let summary = response
        .content
        .unwrap_or_else(|| "[Compaction summary unavailable]".to_string());
    let summary_tokens = summary.len() as u64 / 4;

    // Insert summary as first message
    let summary_msg = Message::system(&format!("[Session compacted] {}", summary));
    session.messages.insert(0, summary_msg);

    info!(
        "Compacted {} messages into summary ({} tokens)",
        compact_count, summary_tokens
    );

    Ok(CompactionResult {
        compacted: true,
        messages_removed: compact_count,
        summary_tokens,
    })
}
