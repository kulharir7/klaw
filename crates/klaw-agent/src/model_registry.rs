//! Provider Model Registry
//! 
//! Dynamic model discovery and capability registry for LLM providers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

/// Model capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCapabilities {
    /// Supports streaming
    pub streaming: bool,
    /// Supports vision/images
    pub vision: bool,
    /// Supports function calling
    pub tools: bool,
    /// Supports JSON mode
    pub json_mode: bool,
    /// Context window size
    pub context_tokens: u64,
    /// Max output tokens
    pub max_output_tokens: u64,
    /// Supports system prompt
    pub system_prompt: bool,
    /// Supports temperature
    pub temperature: bool,
    /// Supports top_p
    pub top_p: bool,
    /// Input cost per 1M tokens
    pub input_cost_per_m: f64,
    /// Output cost per 1M tokens
    pub output_cost_per_m: f64,
}

impl Default for ModelCapabilities {
    fn default() -> Self {
        Self {
            streaming: true,
            vision: false,
            tools: true,
            json_mode: true,
            context_tokens: 8192,
            max_output_tokens: 4096,
            system_prompt: true,
            temperature: true,
            top_p: true,
            input_cost_per_m: 0.0,
            output_cost_per_m: 0.0,
        }
    }
}

/// Model info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model ID (e.g., "anthropic/claude-sonnet-4-20250514")
    pub id: String,
    /// Display name
    pub name: String,
    /// Provider (anthropic, openai, google, etc.)
    pub provider: String,
    /// Model capabilities
    pub capabilities: ModelCapabilities,
    /// Model family
    pub family: String,
    /// Is deprecated?
    pub deprecated: bool,
    /// Replacement model (if deprecated)
    pub replacement: Option<String>,
    /// Tags for filtering
    pub tags: Vec<String>,
}

impl ModelInfo {
    pub fn supports_vision(&self) -> bool { self.capabilities.vision }
    pub fn supports_tools(&self) -> bool { self.capabilities.tools }
    
    pub fn estimate_cost(&self, input_tokens: u64, output_tokens: u64) -> f64 {
        let input_cost = (input_tokens as f64 / 1_000_000.0) * self.capabilities.input_cost_per_m;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * self.capabilities.output_cost_per_m;
        input_cost + output_cost
    }
}

/// Provider Model Registry (sync version)
pub struct ModelRegistry {
    models: RwLock<HashMap<String, ModelInfo>>,
    providers: RwLock<HashMap<String, Vec<String>>>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        let default_models = get_default_models();
        let mut models = HashMap::new();
        let mut providers = HashMap::new();
        
        for model in default_models {
            let provider = model.provider.clone();
            let id = model.id.clone();
            models.insert(id.clone(), model);
            providers.entry(provider)
                .or_insert_with(Vec::new)
                .push(id);
        }
        
        Self {
            models: RwLock::new(models),
            providers: RwLock::new(providers),
        }
    }
    
    /// Register a new model
    pub fn register(&self, model: ModelInfo) {
        let provider = model.provider.clone();
        let id = model.id.clone();
        
        let mut models = self.models.write().unwrap();
        let mut providers = self.providers.write().unwrap();
        
        models.insert(id.clone(), model);
        providers.entry(provider)
            .or_insert_with(Vec::new)
            .push(id);
    }
    
    /// Get model by ID
    pub fn get(&self, id: &str) -> Option<ModelInfo> {
        self.models.read().unwrap().get(id).cloned()
    }
    
    /// List all models
    pub fn list_all(&self) -> Vec<ModelInfo> {
        self.models.read().unwrap().values().cloned().collect()
    }
    
    /// List models by provider
    pub fn list_by_provider(&self, provider: &str) -> Vec<ModelInfo> {
        let models = self.models.read().unwrap();
        let providers = self.providers.read().unwrap();
        
        providers.get(provider)
            .map(|ids| ids.iter().filter_map(|id| models.get(id).cloned()).collect())
            .unwrap_or_default()
    }
    
    /// Find models by capability
    pub fn find_by_capability(&self, capability: &str) -> Vec<ModelInfo> {
        self.models.read().unwrap().values()
            .filter(|m| match capability {
                "vision" => m.capabilities.vision,
                "tools" => m.capabilities.tools,
                "streaming" => m.capabilities.streaming,
                "json_mode" => m.capabilities.json_mode,
                _ => false,
            })
            .cloned()
            .collect()
    }
    
    /// Find cheapest model
    pub fn find_cheapest(&self) -> Option<ModelInfo> {
        self.models.read().unwrap().values()
            .filter(|m| !m.deprecated)
            .min_by(|a, b| {
                (a.capabilities.input_cost_per_m + a.capabilities.output_cost_per_m)
                    .partial_cmp(&(b.capabilities.input_cost_per_m + b.capabilities.output_cost_per_m))
                    .unwrap()
            })
            .cloned()
    }
    
    /// Find largest context
    pub fn find_largest_context(&self) -> Option<ModelInfo> {
        self.models.read().unwrap().values()
            .filter(|m| !m.deprecated)
            .max_by_key(|m| m.capabilities.context_tokens)
            .cloned()
    }
    
    /// Get replacement for deprecated model
    pub fn get_replacement(&self, id: &str) -> Option<ModelInfo> {
        let models = self.models.read().unwrap();
        models.get(id)
            .and_then(|m| m.replacement.as_ref())
            .and_then(|r| models.get(r).cloned())
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Get default known models
fn get_default_models() -> Vec<ModelInfo> {
    vec![
        // Anthropic
        ModelInfo {
            id: "anthropic/claude-sonnet-4-20250514".into(),
            name: "Claude Sonnet 4".into(),
            provider: "anthropic".into(),
            capabilities: ModelCapabilities {
                streaming: true, vision: false, tools: true, json_mode: false,
                context_tokens: 200_000, max_output_tokens: 8192,
                input_cost_per_m: 3.0, output_cost_per_m: 15.0,
                ..Default::default()
            },
            family: "claude".into(), deprecated: false, replacement: None,
            tags: vec!["recommended".into(), "balanced".into()],
        },
        ModelInfo {
            id: "anthropic/claude-opus-4-20250524".into(),
            name: "Claude Opus 4".into(),
            provider: "anthropic".into(),
            capabilities: ModelCapabilities {
                streaming: true, vision: true, tools: true, json_mode: false,
                context_tokens: 200_000, max_output_tokens: 16_384,
                input_cost_per_m: 15.0, output_cost_per_m: 75.0,
                ..Default::default()
            },
            family: "claude".into(), deprecated: false, replacement: None,
            tags: vec!["recommended".into(), "premium".into()],
        },
        ModelInfo {
            id: "anthropic/claude-3.5-haiku".into(),
            name: "Claude 3.5 Haiku".into(),
            provider: "anthropic".into(),
            capabilities: ModelCapabilities {
                streaming: true, vision: false, tools: true, json_mode: false,
                context_tokens: 200_000, max_output_tokens: 8192,
                input_cost_per_m: 0.25, output_cost_per_m: 1.25,
                ..Default::default()
            },
            family: "claude".into(), deprecated: false, replacement: None,
            tags: vec!["recommended".into(), "fast".into()],
        },
        // OpenAI
        ModelInfo {
            id: "openai/gpt-4o".into(),
            name: "GPT-4o".into(),
            provider: "openai".into(),
            capabilities: ModelCapabilities {
                streaming: true, vision: true, tools: true, json_mode: true,
                context_tokens: 128_000, max_output_tokens: 16_384,
                input_cost_per_m: 2.5, output_cost_per_m: 10.0,
                ..Default::default()
            },
            family: "gpt".into(), deprecated: false, replacement: None,
            tags: vec!["recommended".into(), "balanced".into()],
        },
        ModelInfo {
            id: "openai/gpt-4-turbo".into(),
            name: "GPT-4 Turbo".into(),
            provider: "openai".into(),
            capabilities: ModelCapabilities {
                streaming: true, vision: true, tools: true, json_mode: true,
                context_tokens: 128_000, max_output_tokens: 4096,
                input_cost_per_m: 10.0, output_cost_per_m: 30.0,
                ..Default::default()
            },
            family: "gpt".into(), deprecated: true, replacement: Some("openai/gpt-4o".into()),
            tags: vec!["deprecated".into()],
        },
        ModelInfo {
            id: "openai/gpt-3.5-turbo".into(),
            name: "GPT-3.5 Turbo".into(),
            provider: "openai".into(),
            capabilities: ModelCapabilities {
                streaming: true, vision: false, tools: true, json_mode: true,
                context_tokens: 16_385, max_output_tokens: 4096,
                input_cost_per_m: 0.5, output_cost_per_m: 1.5,
                ..Default::default()
            },
            family: "gpt".into(), deprecated: false, replacement: None,
            tags: vec!["fast".into(), "cheap".into()],
        },
        // Google
        ModelInfo {
            id: "google/gemini-1.5-pro".into(),
            name: "Gemini 1.5 Pro".into(),
            provider: "google".into(),
            capabilities: ModelCapabilities {
                streaming: true, vision: true, tools: true, json_mode: true,
                context_tokens: 1_000_000, max_output_tokens: 8192,
                input_cost_per_m: 1.25, output_cost_per_m: 5.0,
                ..Default::default()
            },
            family: "gemini".into(), deprecated: false, replacement: None,
            tags: vec!["recommended".into(), "large-context".into()],
        },
        ModelInfo {
            id: "google/gemini-1.5-flash".into(),
            name: "Gemini 1.5 Flash".into(),
            provider: "google".into(),
            capabilities: ModelCapabilities {
                streaming: true, vision: true, tools: true, json_mode: true,
                context_tokens: 1_000_000, max_output_tokens: 8192,
                input_cost_per_m: 0.075, output_cost_per_m: 0.3,
                ..Default::default()
            },
            family: "gemini".into(), deprecated: false, replacement: None,
            tags: vec!["fast".into(), "cheap".into()],
        },
        // Meta
        ModelInfo {
            id: "meta/llama-3.1-70b-instruct".into(),
            name: "Llama 3.1 70B".into(),
            provider: "meta".into(),
            capabilities: ModelCapabilities {
                streaming: true, vision: false, tools: true, json_mode: false,
                context_tokens: 128_000, max_output_tokens: 4096,
                input_cost_per_m: 0.9, output_cost_per_m: 0.9,
                ..Default::default()
            },
            family: "llama".into(), deprecated: false, replacement: None,
            tags: vec!["open-source".into()],
        },
        // Mistral
        ModelInfo {
            id: "mistral/mistral-large".into(),
            name: "Mistral Large".into(),
            provider: "mistral".into(),
            capabilities: ModelCapabilities {
                streaming: true, vision: false, tools: true, json_mode: true,
                context_tokens: 32_000, max_output_tokens: 4096,
                input_cost_per_m: 2.0, output_cost_per_m: 6.0,
                ..Default::default()
            },
            family: "mistral".into(), deprecated: false, replacement: None,
            tags: vec!["recommended".into()],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_registry_created() {
        let registry = ModelRegistry::new();
        let models = registry.list_all();
        assert!(!models.is_empty());
    }
    
    #[test]
    fn test_get_model() {
        let registry = ModelRegistry::new();
        let model = registry.get("anthropic/claude-sonnet-4-20250514");
        assert!(model.is_some());
        assert_eq!(model.unwrap().provider, "anthropic");
    }
    
    #[test]
    fn test_find_by_capability() {
        let registry = ModelRegistry::new();
        let vision_models = registry.find_by_capability("vision");
        assert!(!vision_models.is_empty());
        assert!(vision_models.iter().all(|m| m.capabilities.vision));
    }
    
    #[test]
    fn test_estimate_cost() {
        let registry = ModelRegistry::new();
        let model = registry.get("anthropic/claude-sonnet-4-20250514").unwrap();
        
        // 100K input, 10K output = $0.30 + $0.15 = $0.45
        let cost = model.estimate_cost(100_000, 10_000);
        assert!((cost - 0.45).abs() < 0.01);
    }
    
    #[test]
    fn test_list_by_provider() {
        let registry = ModelRegistry::new();
        let anthropic = registry.list_by_provider("anthropic");
        assert!(anthropic.len() >= 3);
    }
    
    #[test]
    fn test_find_cheapest() {
        let registry = ModelRegistry::new();
        let cheapest = registry.find_cheapest();
        assert!(cheapest.is_some());
    }
    
    #[test]
    fn test_find_largest_context() {
        let registry = ModelRegistry::new();
        let largest = registry.find_largest_context();
        assert!(largest.is_some());
        assert_eq!(largest.unwrap().capabilities.context_tokens, 1_000_000);
    }
}