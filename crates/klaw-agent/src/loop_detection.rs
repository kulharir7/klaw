use serde_json::Value;
use std::collections::{hash_map::DefaultHasher, VecDeque};
use std::hash::{Hash, Hasher};
use std::time::Instant;

/// Status from loop detection check
#[derive(Debug, Clone)]
pub enum LoopStatus {
    Ok,
    Warning(String),
    Critical(String),
    CircuitBreaker(String),
}

struct ToolCallRecord {
    tool_name: String,
    params_hash: u64,
    #[allow(dead_code)]
    timestamp: Instant,
}

/// Detects repetitive tool call patterns
pub struct LoopDetector {
    history: VecDeque<ToolCallRecord>,
    max_history: usize,
}

impl LoopDetector {
    pub fn new(max_history: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_history),
            max_history,
        }
    }

    fn hash_params(params: &Value) -> u64 {
        let mut hasher = DefaultHasher::new();
        let s = params.to_string();
        s.hash(&mut hasher);
        hasher.finish()
    }

    /// Record a tool call
    pub fn record(&mut self, name: &str, params: &Value) {
        if self.history.len() >= self.max_history {
            self.history.pop_front();
        }
        self.history.push_back(ToolCallRecord {
            tool_name: name.to_string(),
            params_hash: Self::hash_params(params),
            timestamp: Instant::now(),
        });
    }

    /// Check for loops
    pub fn check(&self) -> LoopStatus {
        if self.history.len() < 3 {
            return LoopStatus::Ok;
        }

        // genericRepeat: same tool+params 3+ times in last 10
        let window: Vec<_> = self.history.iter().rev().take(10).collect();
        if let Some(last) = window.first() {
            let repeat_count = window
                .iter()
                .filter(|r| r.tool_name == last.tool_name && r.params_hash == last.params_hash)
                .count();

            if repeat_count >= 5 {
                return LoopStatus::CircuitBreaker(format!(
                    "Tool '{}' called {} times with identical params — circuit breaker triggered",
                    last.tool_name, repeat_count
                ));
            }
            if repeat_count >= 3 {
                return LoopStatus::Critical(format!(
                    "Tool '{}' called {} times with identical params",
                    last.tool_name, repeat_count
                ));
            }
        }

        // pingPong: A→B→A→B pattern
        if self.history.len() >= 4 {
            let h: Vec<_> = self.history.iter().rev().take(4).collect();
            if h[0].tool_name == h[2].tool_name
                && h[1].tool_name == h[3].tool_name
                && h[0].tool_name != h[1].tool_name
            {
                return LoopStatus::Warning(format!(
                    "Ping-pong detected: {} ↔ {}",
                    h[0].tool_name, h[1].tool_name
                ));
            }
        }

        LoopStatus::Ok
    }
}
