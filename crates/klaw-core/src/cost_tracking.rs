//! Provider Cost Tracking
//! 
//! Track token usage and costs across providers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use chrono::{DateTime, Utc};

/// Usage record for a single request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub session_key: String,
    pub agent_id: String,
    pub model: String,
    pub provider: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub input_cost: f64,
    pub output_cost: f64,
    pub total_cost: f64,
    pub request_type: String,
    pub latency_ms: u64,
    pub success: bool,
}

/// Aggregated usage stats
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageStats {
    pub total_requests: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cost: f64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub avg_latency_ms: f64,
    pub by_provider: HashMap<String, ProviderStats>,
    pub by_model: HashMap<String, ModelStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderStats {
    pub requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost: f64,
    pub errors: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelStats {
    pub requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost: f64,
}

/// Cost tracking store
pub struct CostTracker {
    records: RwLock<Vec<UsageRecord>>,
    model_costs: RwLock<HashMap<String, ModelCostConfig>>,
}

/// Cost configuration per model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCostConfig {
    pub input_cost_per_m: f64,
    pub output_cost_per_m: f64,
}

impl CostTracker {
    pub fn new() -> Self {
        Self {
            records: RwLock::new(Vec::new()),
            model_costs: RwLock::new(Self::default_costs()),
        }
    }
    
    fn default_costs() -> HashMap<String, ModelCostConfig> {
        let mut costs = HashMap::new();
        
        // Anthropic
        costs.insert(
            "anthropic/claude-sonnet-4-20250514".into(),
            ModelCostConfig { input_cost_per_m: 3.0, output_cost_per_m: 15.0 }
        );
        costs.insert(
            "anthropic/claude-opus-4-20250524".into(),
            ModelCostConfig { input_cost_per_m: 15.0, output_cost_per_m: 75.0 }
        );
        costs.insert(
            "anthropic/claude-3.5-haiku".into(),
            ModelCostConfig { input_cost_per_m: 0.25, output_cost_per_m: 1.25 }
        );
        
        // OpenAI
        costs.insert(
            "openai/gpt-4o".into(),
            ModelCostConfig { input_cost_per_m: 2.5, output_cost_per_m: 10.0 }
        );
        costs.insert(
            "openai/gpt-4-turbo".into(),
            ModelCostConfig { input_cost_per_m: 10.0, output_cost_per_m: 30.0 }
        );
        costs.insert(
            "openai/gpt-3.5-turbo".into(),
            ModelCostConfig { input_cost_per_m: 0.5, output_cost_per_m: 1.5 }
        );
        
        // Google
        costs.insert(
            "google/gemini-1.5-pro".into(),
            ModelCostConfig { input_cost_per_m: 1.25, output_cost_per_m: 5.0 }
        );
        costs.insert(
            "google/gemini-1.5-flash".into(),
            ModelCostConfig { input_cost_per_m: 0.075, output_cost_per_m: 0.3 }
        );
        
        // Meta
        costs.insert(
            "meta/llama-3.1-70b-instruct".into(),
            ModelCostConfig { input_cost_per_m: 0.9, output_cost_per_m: 0.9 }
        );
        
        // Mistral
        costs.insert(
            "mistral/mistral-large".into(),
            ModelCostConfig { input_cost_per_m: 2.0, output_cost_per_m: 6.0 }
        );
        
        costs
    }
    
    /// Record usage
    pub fn record(&self, record: UsageRecord) {
        let mut records = self.records.write().unwrap();
        records.push(record);
        
        // Keep last 10k records in memory
        if records.len() > 10000 {
            records.remove(0);
        }
    }
    
    /// Create a usage record with auto-calculated cost
    pub fn create_record(
        &self,
        session_key: &str,
        agent_id: &str,
        model: &str,
        provider: &str,
        input_tokens: u64,
        output_tokens: u64,
        request_type: &str,
        latency_ms: u64,
        success: bool,
    ) -> UsageRecord {
        let model_costs = self.model_costs.read().unwrap();
        let costs = model_costs.get(model)
            .or_else(|| model_costs.get(&format!("{}/{}", provider, model)))
            .map(|c| (c.input_cost_per_m, c.output_cost_per_m))
            .unwrap_or((0.0, 0.0));
        
        let input_cost = (input_tokens as f64 / 1_000_000.0) * costs.0;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * costs.1;
        
        UsageRecord {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            session_key: session_key.to_string(),
            agent_id: agent_id.to_string(),
            model: model.to_string(),
            provider: provider.to_string(),
            input_tokens,
            output_tokens,
            input_cost,
            output_cost,
            total_cost: input_cost + output_cost,
            request_type: request_type.to_string(),
            latency_ms,
            success,
        }
    }
    
    /// Get aggregated stats
    pub fn get_stats(&self) -> UsageStats {
        let records = self.records.read().unwrap();
        let mut stats = UsageStats::default();
        
        for record in records.iter() {
            stats.total_requests += 1;
            stats.total_input_tokens += record.input_tokens;
            stats.total_output_tokens += record.output_tokens;
            stats.total_cost += record.total_cost;
            
            if record.success {
                stats.successful_requests += 1;
            } else {
                stats.failed_requests += 1;
            }
            
            // Provider stats
            let provider_stats = stats.by_provider
                .entry(record.provider.clone())
                .or_insert_with(ProviderStats::default);
            provider_stats.requests += 1;
            provider_stats.input_tokens += record.input_tokens;
            provider_stats.output_tokens += record.output_tokens;
            provider_stats.cost += record.total_cost;
            if !record.success {
                provider_stats.errors += 1;
            }
            
            // Model stats
            let model_stats = stats.by_model
                .entry(record.model.clone())
                .or_insert_with(ModelStats::default);
            model_stats.requests += 1;
            model_stats.input_tokens += record.input_tokens;
            model_stats.output_tokens += record.output_tokens;
            model_stats.cost += record.total_cost;
        }
        
        if !records.is_empty() {
            stats.avg_latency_ms = records.iter()
                .map(|r| r.latency_ms as f64)
                .sum::<f64>() / records.len() as f64;
        }
        
        stats
    }
    
    /// Get stats for a session
    pub fn get_session_stats(&self, session_key: &str) -> Option<UsageStats> {
        let records = self.records.read().unwrap();
        let session_records: Vec<_> = records.iter()
            .filter(|r| r.session_key == session_key)
            .collect();
        
        if session_records.is_empty() {
            return None;
        }
        
        let mut stats = UsageStats::default();
        
        for record in session_records {
            stats.total_requests += 1;
            stats.total_input_tokens += record.input_tokens;
            stats.total_output_tokens += record.output_tokens;
            stats.total_cost += record.total_cost;
            
            if record.success {
                stats.successful_requests += 1;
            } else {
                stats.failed_requests += 1;
            }
        }
        
        Some(stats)
    }
    
    /// Get records in time range
    pub fn get_records_in_range(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<UsageRecord> {
        let records = self.records.read().unwrap();
        records.iter()
            .filter(|r| r.timestamp >= start && r.timestamp <= end)
            .cloned()
            .collect()
    }
    
    /// Set cost for a model
    pub fn set_model_cost(&self, model: &str, input_per_m: f64, output_per_m: f64) {
        let mut costs = self.model_costs.write().unwrap();
        costs.insert(model.to_string(), ModelCostConfig {
            input_cost_per_m: input_per_m,
            output_cost_per_m: output_per_m,
        });
    }
    
    /// Export records as CSV
    pub fn export_csv(&self) -> String {
        let records = self.records.read().unwrap();
        let mut csv = String::from("id,timestamp,session_key,model,provider,input_tokens,output_tokens,total_cost,latency_ms,success\n");
        
        for r in records.iter() {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{}\n",
                r.id,
                r.timestamp.to_rfc3339(),
                r.session_key,
                r.model,
                r.provider,
                r.input_tokens,
                r.output_tokens,
                r.total_cost,
                r.latency_ms,
                r.success
            ));
        }
        
        csv
    }
}

impl Default for CostTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_record() {
        let tracker = CostTracker::new();
        let record = tracker.create_record(
            "session-123",
            "agent-1",
            "anthropic/claude-sonnet-4-20250514",
            "anthropic",
            1000,
            500,
            "chat",
            1500,
            true
        );
        
        assert_eq!(record.input_tokens, 1000);
        assert_eq!(record.output_tokens, 500);
        // Input: 1000/1M * $3 = $0.003
        // Output: 500/1M * $15 = $0.0075
        // Total: $0.0105
        assert!((record.total_cost - 0.0105).abs() < 0.0001);
    }
    
    #[test]
    fn test_record_and_stats() {
        let tracker = CostTracker::new();
        
        let record = tracker.create_record(
            "session-1",
            "agent-1",
            "openai/gpt-4o",
            "openai",
            1000,
            500,
            "chat",
            500,
            true
        );
        
        tracker.record(record);
        
        let stats = tracker.get_stats();
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.total_input_tokens, 1000);
        assert!(stats.by_provider.contains_key("openai"));
        assert!(stats.by_model.contains_key("openai/gpt-4o"));
    }
    
    #[test]
    fn test_session_stats() {
        let tracker = CostTracker::new();
        
        // Add a record
        let record = tracker.create_record(
            "session-1",
            "agent-1",
            "openai/gpt-4o",
            "openai",
            1000,
            500,
            "chat",
            500,
            true
        );
        tracker.record(record);
        
        // Different session - should not appear
        let record2 = tracker.create_record(
            "session-2",
            "agent-1",
            "openai/gpt-4o",
            "openai",
            2000,
            1000,
            "chat",
            600,
            true
        );
        tracker.record(record2);
        
        let session_stats = tracker.get_session_stats("session-1").unwrap();
        assert_eq!(session_stats.total_requests, 1);
        assert_eq!(session_stats.total_input_tokens, 1000);
    }
    
    #[test]
    fn test_custom_cost() {
        let tracker = CostTracker::new();
        
        // Set custom cost
        tracker.set_model_cost("custom-model", 1.0, 2.0);
        
        let record = tracker.create_record(
            "session-1",
            "agent-1",
            "custom-model",
            "custom",
            1000,
            500,
            "chat",
            100,
            true
        );
        
        // Input: 1000/1M * $1 = $0.001
        // Output: 500/1M * $2 = $0.001
        // Total: $0.002
        assert!((record.total_cost - 0.002).abs() < 0.0001);
    }
    
    #[test]
    fn test_export_csv() {
        let tracker = CostTracker::new();
        
        let record = tracker.create_record(
            "session-1",
            "agent-1",
            "openai/gpt-4o",
            "openai",
            1000,
            500,
            "chat",
            500,
            true
        );
        tracker.record(record);
        
        let csv = tracker.export_csv();
        assert!(csv.contains("session-1"));
        assert!(csv.contains("openai/gpt-4o"));
        assert!(csv.contains("1000"));
    }
}