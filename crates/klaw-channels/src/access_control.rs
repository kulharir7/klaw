use std::collections::{HashMap, HashSet};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DmPolicy {
    Pairing,
    Allowlist,
    #[default]
    Open,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GroupPolicy {
    Allowlist,
    #[default]
    Open,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccessControl {
    pub dm_policy: DmPolicy,
    pub allow_from: Vec<String>,
    pub group_policy: GroupPolicy,
}

impl AccessControl {
    /// Check if a DM sender is allowed
    pub fn check_dm(&self, sender_id: &str, paired_store: &HashSet<String>) -> bool {
        match self.dm_policy {
            DmPolicy::Open => true,
            DmPolicy::Disabled => false,
            DmPolicy::Allowlist => self.allow_from.iter().any(|id| id == sender_id),
            DmPolicy::Pairing => paired_store.contains(sender_id),
        }
    }

    /// Check if a group is allowed
    pub fn check_group(&self, group_id: &str, allowed_groups: &HashSet<String>) -> bool {
        match self.group_policy {
            GroupPolicy::Open => true,
            GroupPolicy::Disabled => false,
            GroupPolicy::Allowlist => allowed_groups.contains(group_id),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingRequest {
    pub code: String,
    pub sender_id: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// Generate a 6-digit pairing code (expires in 1 hour)
pub fn generate_pairing_code() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("{:06}", seed % 1_000_000)
}

/// Verify a pairing code against pending requests
pub fn verify_pairing_code(code: &str, pending: &HashMap<String, PairingRequest>) -> bool {
    let now = Utc::now();
    pending.values().any(|req| req.code == code && req.expires_at > now)
}
