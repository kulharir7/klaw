use klaw_core::config::HeartbeatConfig;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::info;

pub struct HeartbeatRunner {
    pub interval: Duration,
    pub model: Option<String>,
    pub session_key: String,
    pub prompt: String,
}

impl HeartbeatRunner {
    /// Create from a HeartbeatConfig (from agent entry)
    pub fn from_heartbeat_config(hb: &HeartbeatConfig) -> Option<Self> {
        let every = hb.every.as_deref()?;
        let interval = parse_duration(every)?;

        let prompt = hb.prompt.clone().unwrap_or_else(|| {
            "Read HEARTBEAT.md if it exists. Follow it strictly. If nothing needs attention, reply HEARTBEAT_OK.".to_string()
        });

        Some(Self {
            interval,
            model: hb.model.clone(),
            session_key: hb.session.clone().unwrap_or_else(|| "heartbeat:main".to_string()),
            prompt,
        })
    }

    /// Run periodic heartbeat — sends prompt via tx for agent processing
    pub async fn run(&self, tx: mpsc::Sender<String>) {
        info!(
            "Heartbeat started: interval={}s, model={:?}",
            self.interval.as_secs(),
            self.model
        );

        let mut interval = tokio::time::interval(self.interval);
        // Skip first immediate tick
        interval.tick().await;

        loop {
            interval.tick().await;
            info!("Heartbeat tick — sending prompt");
            if tx.send(self.prompt.clone()).await.is_err() {
                info!("Heartbeat channel closed, stopping");
                break;
            }
        }
    }
}

/// Parse duration strings like "30m", "1h", "60s", "2h30m"
fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    let mut total_secs: u64 = 0;
    let mut num_buf = String::new();

    for c in s.chars() {
        if c.is_ascii_digit() {
            num_buf.push(c);
        } else {
            let n: u64 = num_buf.parse().ok()?;
            num_buf.clear();
            match c {
                's' => total_secs += n,
                'm' => total_secs += n * 60,
                'h' => total_secs += n * 3600,
                'd' => total_secs += n * 86400,
                _ => return None,
            }
        }
    }

    // If only digits, assume seconds
    if !num_buf.is_empty() {
        total_secs += num_buf.parse::<u64>().ok()?;
    }

    if total_secs > 0 { Some(Duration::from_secs(total_secs)) } else { None }
}
