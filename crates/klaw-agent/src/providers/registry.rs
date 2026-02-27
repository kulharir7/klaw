use crate::provider::LlmProvider;
use crate::providers::openai::OpenAiProvider;
use crate::providers::anthropic::AnthropicProvider;
use std::collections::HashMap;
use tracing::info;

/// Provider API type
#[derive(Debug, Clone, PartialEq)]
pub enum ApiType {
    OpenAiCompletions,
    AnthropicMessages,
}

/// Provider definition
#[derive(Debug, Clone)]
pub struct ProviderDef {
    pub name: String,
    pub base_url: String,
    pub api_type: ApiType,
    pub env_key: String,          // Primary env var for API key
    pub env_keys_alt: Vec<String>, // Alternative env vars
    pub auto_discover: bool,       // Auto-detect local servers
}

/// Build the full provider catalog (all 30+ providers)
pub fn built_in_providers() -> HashMap<String, ProviderDef> {
    let mut p = HashMap::new();

    // === Core Providers ===
    p.insert("openai".into(), ProviderDef {
        name: "OpenAI".into(),
        base_url: "https://api.openai.com/v1".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "OPENAI_API_KEY".into(),
        env_keys_alt: vec!["OPENAI_API_KEYS".into()],
        auto_discover: false,
    });

    p.insert("anthropic".into(), ProviderDef {
        name: "Anthropic".into(),
        base_url: "https://api.anthropic.com/v1".into(),
        api_type: ApiType::AnthropicMessages,
        env_key: "ANTHROPIC_API_KEY".into(),
        env_keys_alt: vec!["ANTHROPIC_API_KEYS".into()],
        auto_discover: false,
    });

    // === Google ===
    p.insert("google".into(), ProviderDef {
        name: "Google Gemini".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta/openai".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "GEMINI_API_KEY".into(),
        env_keys_alt: vec!["GOOGLE_API_KEY".into(), "GEMINI_API_KEYS".into()],
        auto_discover: false,
    });

    p.insert("google-vertex".into(), ProviderDef {
        name: "Google Vertex AI".into(),
        base_url: "https://us-central1-aiplatform.googleapis.com/v1beta1/openai".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "GOOGLE_APPLICATION_CREDENTIALS".into(),
        env_keys_alt: vec![],
        auto_discover: false,
    });

    // === Meta/Open Source Gateways ===
    p.insert("openrouter".into(), ProviderDef {
        name: "OpenRouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "OPENROUTER_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: false,
    });

    p.insert("groq".into(), ProviderDef {
        name: "Groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "GROQ_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: false,
    });

    p.insert("cerebras".into(), ProviderDef {
        name: "Cerebras".into(),
        base_url: "https://api.cerebras.ai/v1".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "CEREBRAS_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: false,
    });

    p.insert("together".into(), ProviderDef {
        name: "Together AI".into(),
        base_url: "https://api.together.xyz/v1".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "TOGETHER_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: false,
    });

    // === xAI / Grok ===
    p.insert("xai".into(), ProviderDef {
        name: "xAI (Grok)".into(),
        base_url: "https://api.x.ai/v1".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "XAI_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: false,
    });

    // === Mistral ===
    p.insert("mistral".into(), ProviderDef {
        name: "Mistral".into(),
        base_url: "https://api.mistral.ai/v1".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "MISTRAL_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: false,
    });

    // === GitHub Copilot ===
    p.insert("github-copilot".into(), ProviderDef {
        name: "GitHub Copilot".into(),
        base_url: "https://api.githubcopilot.com".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "COPILOT_GITHUB_TOKEN".into(),
        env_keys_alt: vec!["GH_TOKEN".into(), "GITHUB_TOKEN".into()],
        auto_discover: false,
    });

    // === Hugging Face ===
    p.insert("huggingface".into(), ProviderDef {
        name: "Hugging Face".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "HUGGINGFACE_HUB_TOKEN".into(),
        env_keys_alt: vec!["HF_TOKEN".into()],
        auto_discover: false,
    });

    // === Gateway Proxies ===
    p.insert("opencode".into(), ProviderDef {
        name: "OpenCode Zen".into(),
        base_url: "https://opencode.ai/api/v1".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "OPENCODE_API_KEY".into(),
        env_keys_alt: vec!["OPENCODE_ZEN_API_KEY".into()],
        auto_discover: false,
    });

    p.insert("kilocode".into(), ProviderDef {
        name: "Kilo Gateway".into(),
        base_url: "https://api.kilo.ai/api/gateway".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "KILOCODE_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: false,
    });

    p.insert("vercel-ai-gateway".into(), ProviderDef {
        name: "Vercel AI Gateway".into(),
        base_url: "https://api.vercel.ai/v1".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "AI_GATEWAY_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: false,
    });

    p.insert("cloudflare".into(), ProviderDef {
        name: "Cloudflare AI Gateway".into(),
        base_url: "https://gateway.ai.cloudflare.com/v1".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "CLOUDFLARE_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: false,
    });

    p.insert("litellm".into(), ProviderDef {
        name: "LiteLLM".into(),
        base_url: "http://localhost:4000/v1".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "LITELLM_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: true,
    });

    // === Venice AI ===
    p.insert("venice".into(), ProviderDef {
        name: "Venice AI".into(),
        base_url: "https://api.venice.ai/api/v1".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "VENICE_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: false,
    });

    // === China/Asia Providers ===
    p.insert("zai".into(), ProviderDef {
        name: "Z.AI (GLM)".into(),
        base_url: "https://open.bigmodel.cn/api/paas/v4".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "ZAI_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: false,
    });

    p.insert("moonshot".into(), ProviderDef {
        name: "Moonshot (Kimi)".into(),
        base_url: "https://api.moonshot.ai/v1".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "MOONSHOT_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: false,
    });

    p.insert("kimi-coding".into(), ProviderDef {
        name: "Kimi Coding".into(),
        base_url: "https://api.moonshot.ai/anthropic/v1".into(),
        api_type: ApiType::AnthropicMessages,
        env_key: "KIMI_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: false,
    });

    p.insert("qianfan".into(), ProviderDef {
        name: "Qianfan (Baidu)".into(),
        base_url: "https://qianfan.baidubce.com/v2".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "QIANFAN_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: false,
    });

    p.insert("nvidia".into(), ProviderDef {
        name: "NVIDIA".into(),
        base_url: "https://integrate.api.nvidia.com/v1".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "NVIDIA_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: false,
    });

    p.insert("xiaomi".into(), ProviderDef {
        name: "Xiaomi".into(),
        base_url: "https://api.xiaomi.com/v1".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "XIAOMI_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: false,
    });

    p.insert("minimax".into(), ProviderDef {
        name: "MiniMax".into(),
        base_url: "https://api.minimax.chat/v1".into(),
        api_type: ApiType::AnthropicMessages,
        env_key: "MINIMAX_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: false,
    });

    p.insert("volcengine".into(), ProviderDef {
        name: "Volcano Engine (Doubao)".into(),
        base_url: "https://ark.cn-beijing.volces.com/api/v3".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "VOLCANO_ENGINE_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: false,
    });

    p.insert("byteplus".into(), ProviderDef {
        name: "BytePlus".into(),
        base_url: "https://ark.us-east-1.bytepluses.com/api/v3".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "BYTEPLUS_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: false,
    });

    p.insert("synthetic".into(), ProviderDef {
        name: "Synthetic".into(),
        base_url: "https://api.synthetic.new/anthropic".into(),
        api_type: ApiType::AnthropicMessages,
        env_key: "SYNTHETIC_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: false,
    });

    // === Amazon Bedrock ===
    p.insert("bedrock".into(), ProviderDef {
        name: "Amazon Bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        api_type: ApiType::AnthropicMessages,
        env_key: "AWS_ACCESS_KEY_ID".into(),
        env_keys_alt: vec!["AWS_SECRET_ACCESS_KEY".into()],
        auto_discover: false,
    });

    // === Local ===
    p.insert("ollama".into(), ProviderDef {
        name: "Ollama".into(),
        base_url: "http://127.0.0.1:11434/v1".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "OLLAMA_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: true,
    });

    p.insert("vllm".into(), ProviderDef {
        name: "vLLM".into(),
        base_url: "http://127.0.0.1:8000/v1".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "VLLM_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: true,
    });

    p.insert("lmstudio".into(), ProviderDef {
        name: "LM Studio".into(),
        base_url: "http://localhost:1234/v1".into(),
        api_type: ApiType::OpenAiCompletions,
        env_key: "LMSTUDIO_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: true,
    });

    // === Transcription ===
    p.insert("deepgram".into(), ProviderDef {
        name: "Deepgram".into(),
        base_url: "https://api.deepgram.com/v1".into(),
        api_type: ApiType::OpenAiCompletions, // placeholder
        env_key: "DEEPGRAM_API_KEY".into(),
        env_keys_alt: vec![],
        auto_discover: false,
    });

    p
}

/// Resolve API key for a provider (checks env vars in priority order)
pub fn resolve_api_key(def: &ProviderDef, config_key: Option<&str>) -> Option<String> {
    // 1. Config-provided key (highest priority)
    if let Some(k) = config_key {
        if !k.is_empty() && k != "YOUR_KEY_HERE" {
            return Some(k.to_string());
        }
    }

    // 2. Live override env var
    let live_var = format!("KLAW_LIVE_{}_KEY", def.name.to_uppercase().replace(' ', "_").replace('(', "").replace(')', ""));
    if let Ok(v) = std::env::var(&live_var) {
        if !v.is_empty() { return Some(v); }
    }

    // 3. Primary env var
    if let Ok(v) = std::env::var(&def.env_key) {
        if !v.is_empty() { return Some(v); }
    }

    // 4. Alternative env vars
    for alt in &def.env_keys_alt {
        if let Ok(v) = std::env::var(alt) {
            if !v.is_empty() { return Some(v); }
        }
    }

    // 5. For auto-discover (local) providers, use empty key
    if def.auto_discover {
        return Some(String::new());
    }

    None
}

/// Create an LLM provider from a provider/model string (e.g., "anthropic/claude-opus-4-6")
pub fn create_provider(
    provider_model: &str,
    config_key: Option<&str>,
    config_base_url: Option<&str>,
    custom_providers: &HashMap<String, ProviderDef>,
) -> anyhow::Result<(Box<dyn LlmProvider>, String)> {
    // Parse "provider/model" format
    let (provider_name, model) = if let Some(idx) = provider_model.find('/') {
        (&provider_model[..idx], &provider_model[idx + 1..])
    } else {
        // No provider prefix — try to guess
        ("anthropic", provider_model)
    };

    // Normalize aliases
    let provider_name = match provider_name {
        "z.ai" | "z-ai" => "zai",
        _ => provider_name,
    };

    // Look up provider definition
    let catalog = built_in_providers();
    let def = custom_providers.get(provider_name)
        .or_else(|| catalog.get(provider_name))
        .ok_or_else(|| anyhow::anyhow!("Unknown provider: '{}'. Use provider/model format.", provider_name))?;

    // Resolve API key
    let api_key = resolve_api_key(def, config_key)
        .ok_or_else(|| anyhow::anyhow!(
            "No API key for provider '{}'. Set {} env var or add api_key to config.",
            provider_name, def.env_key
        ))?;

    // Resolve base URL (config override > provider default)
    let base_url = config_base_url.unwrap_or(&def.base_url);

    info!("Creating provider: {} ({}), model: {}", def.name, provider_name, model);

    // Create the appropriate provider based on API type
    let provider: Box<dyn LlmProvider> = match def.api_type {
        ApiType::OpenAiCompletions => {
            Box::new(OpenAiProvider::new(base_url, &api_key, provider_name))
        }
        ApiType::AnthropicMessages => {
            Box::new(AnthropicProvider::new(&api_key))
        }
    };

    Ok((provider, model.to_string()))
}

/// List all available providers
pub fn list_providers() -> Vec<(String, String, String)> {
    let catalog = built_in_providers();
    let mut list: Vec<(String, String, String)> = catalog.iter()
        .map(|(id, def)| (id.clone(), def.name.clone(), def.env_key.clone()))
        .collect();
    list.sort_by(|a, b| a.0.cmp(&b.0));
    list
}
