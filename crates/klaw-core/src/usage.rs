use std::collections::HashMap;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct UsageTracker {
    pub sessions: HashMap<String, SessionUsage>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SessionUsage {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cost_usd: f64,
    pub model_usage: HashMap<String, ModelUsage>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ModelUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    pub calls: u32,
}

impl UsageTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(
        &mut self,
        session_id: &str,
        provider: &str,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
    ) {
        let cost = estimate_cost(provider, model, input_tokens, output_tokens);
        let session = self.sessions.entry(session_id.to_string()).or_default();
        session.total_input_tokens += input_tokens;
        session.total_output_tokens += output_tokens;
        session.total_cost_usd += cost;

        let model_key = format!("{}/{}", provider, model);
        let model_usage = session.model_usage.entry(model_key).or_default();
        model_usage.input_tokens += input_tokens;
        model_usage.output_tokens += output_tokens;
        model_usage.cost_usd += cost;
        model_usage.calls += 1;
    }

    pub fn get_session(&self, session_id: &str) -> Option<&SessionUsage> {
        self.sessions.get(session_id)
    }

    pub fn total_cost(&self) -> f64 {
        self.sessions.values().map(|s| s.total_cost_usd).sum()
    }
}

/// Rough cost estimation per 1M tokens
pub fn estimate_cost(provider: &str, model: &str, input_tokens: u64, output_tokens: u64) -> f64 {
    let (input_rate, output_rate) = match (provider, model) {
        ("anthropic", m) if m.contains("opus") => (15.0, 75.0),
        ("anthropic", m) if m.contains("sonnet") => (3.0, 15.0),
        ("anthropic", m) if m.contains("haiku") => (0.25, 1.25),
        ("openai", m) if m.contains("gpt-4o-mini") => (0.15, 0.6),
        ("openai", m) if m.contains("gpt-4o") => (2.5, 10.0),
        ("openai", m) if m.contains("o1") => (15.0, 60.0),
        ("google", m) if m.contains("gemini") => (0.5, 1.5),
        _ => (1.0, 3.0), // Default
    };

    let input_cost = (input_tokens as f64 / 1_000_000.0) * input_rate;
    let output_cost = (output_tokens as f64 / 1_000_000.0) * output_rate;
    input_cost + output_cost
}
