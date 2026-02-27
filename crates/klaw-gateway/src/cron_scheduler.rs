use chrono::{DateTime, Utc, Timelike, Datelike};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub id: String,
    pub schedule: String,
    pub task: String,
    pub enabled: bool,
    pub last_run: Option<DateTime<Utc>>,
    pub next_run: Option<DateTime<Utc>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CronScheduler {
    jobs: Vec<CronJob>,
    #[serde(skip)]
    running: bool,
}

impl CronScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_jobs() -> anyhow::Result<Self> {
        let path = klaw_core::Config::home_dir().join("cron_jobs.json");
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let scheduler: CronScheduler = serde_json::from_str(&content)?;
            info!("Loaded {} cron jobs", scheduler.jobs.len());
            Ok(scheduler)
        } else {
            Ok(Self::new())
        }
    }

    pub fn add_job(&mut self, schedule: &str, task: &str) -> String {
        let id = uuid::Uuid::new_v4().to_string()[..8].to_string();
        self.jobs.push(CronJob {
            id: id.clone(),
            schedule: schedule.to_string(),
            task: task.to_string(),
            enabled: true,
            last_run: None,
            next_run: None,
        });
        id
    }

    pub fn remove_job(&mut self, id: &str) -> bool {
        let len_before = self.jobs.len();
        self.jobs.retain(|j| j.id != id);
        self.jobs.len() < len_before
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = klaw_core::Config::home_dir().join("cron_jobs.json");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    /// Start the scheduler loop
    pub async fn run(&mut self, tx: mpsc::Sender<String>) {
        self.running = true;
        info!("Cron scheduler started with {} jobs", self.jobs.len());

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        while self.running {
            interval.tick().await;
            let now = Utc::now();

            for job in &mut self.jobs {
                if !job.enabled {
                    continue;
                }
                if matches_cron(&job.schedule, &now) {
                    info!("Cron job '{}' triggered: {}", job.id, job.task);
                    job.last_run = Some(now);
                    if tx.send(job.task.clone()).await.is_err() {
                        warn!("Cron task channel closed");
                        self.running = false;
                        break;
                    }
                }
            }
        }
    }
}

/// Simple cron pattern matcher: "min hour dom month dow"
/// Supports: `*`, specific numbers, `*/N` (step)
fn matches_cron(schedule: &str, now: &DateTime<Utc>) -> bool {
    let parts: Vec<&str> = schedule.split_whitespace().collect();
    if parts.len() != 5 {
        return false;
    }

    let checks = [
        (parts[0], now.minute()),
        (parts[1], now.hour()),
        (parts[2], now.day()),
        (parts[3], now.month()),
        (parts[4], now.weekday().num_days_from_sunday()),
    ];

    checks.iter().all(|(pattern, value)| matches_field(pattern, *value))
}

fn matches_field(pattern: &str, value: u32) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(step) = pattern.strip_prefix("*/") {
        if let Ok(n) = step.parse::<u32>() {
            return n > 0 && value % n == 0;
        }
    }
    if let Ok(exact) = pattern.parse::<u32>() {
        return value == exact;
    }
    // Comma-separated values
    if pattern.contains(',') {
        return pattern.split(',').any(|p| {
            p.trim().parse::<u32>().map(|v| v == value).unwrap_or(false)
        });
    }
    false
}
