use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::info;

// ─── Model Spec ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ModelSpec {
    Simple(String),
    WithFallbacks {
        primary: String,
        fallbacks: Vec<String>,
    },
}

// ─── Small Config Structs ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct HumanDelayConfig {
    pub mode: Option<String>,
    pub min_ms: Option<u64>,
    pub max_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SandboxConfig {
    pub mode: Option<String>,
    pub scope: Option<String>,
    pub workspace_access: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SubagentsConfig {
    pub allow_agents: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct GroupChatConfig {
    pub history_limit: Option<u32>,
    pub mention_patterns: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct HeartbeatConfig {
    pub every: Option<String>,
    pub model: Option<String>,
    pub session: Option<String>,
    pub target: Option<String>,
    pub prompt: Option<String>,
    pub ack_max_chars: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CompactionConfig {
    pub mode: Option<String>,
    pub reserve_tokens_floor: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ContextPruningConfig {
    pub mode: Option<String>,
    pub ttl: Option<String>,
    pub keep_last_assistants: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AgentIdentity {
    pub name: Option<String>,
    pub theme: Option<String>,
    pub emoji: Option<String>,
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RetryConfig {
    pub attempts: Option<u32>,
    pub min_delay_ms: Option<u64>,
    pub max_delay_ms: Option<u64>,
    pub jitter: Option<f32>,
}

// ─── Binding ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Binding {
    pub agent_id: String,
    #[serde(flatten)]
    pub match_rule: BindingMatch,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct BindingMatch {
    pub channel: Option<String>,
    pub account_id: Option<String>,
    pub peer: Option<serde_json::Value>,
    pub guild_id: Option<String>,
    pub team_id: Option<String>,
}

// ─── Session Sub-configs ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SessionResetConfig {
    pub mode: Option<String>,
    pub at_hour: Option<u32>,
    pub idle_minutes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SessionMaintenanceConfig {
    pub mode: Option<String>,
    pub prune_after: Option<String>,
    pub max_entries: Option<u32>,
    pub rotate_bytes: Option<String>,
    pub max_disk_bytes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ThreadBindingsConfig {
    pub enabled: Option<bool>,
    pub ttl_hours: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AgentToAgentConfig {
    pub max_ping_pong_turns: Option<u32>,
}

// ─── Tools Sub-configs ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LoopDetectionConfig {
    pub enabled: bool,
    pub warning_threshold: Option<u32>,
    pub critical_threshold: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ElevatedConfig {
    pub enabled: bool,
    pub allow_from: Option<HashMap<String, Vec<String>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct WebToolsConfig {
    pub search: Option<WebSearchConfig>,
    pub fetch: Option<WebFetchConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct WebSearchConfig {
    pub enabled: Option<bool>,
    pub max_results: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct WebFetchConfig {
    pub enabled: Option<bool>,
    pub max_chars_cap: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ExecToolsConfig {
    pub apply_patch: Option<ApplyPatchConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ApplyPatchConfig {
    pub enabled: Option<bool>,
    pub workspace_only: Option<bool>,
}

// ─── Channel Sub-configs ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ChannelDefaults {
    pub group_policy: Option<String>,
    pub heartbeat: Option<ChannelHeartbeatConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ChannelHeartbeatConfig {
    pub show_ok: Option<bool>,
    pub show_alerts: Option<bool>,
    pub use_indicator: Option<bool>,
}

// ─── Commands / Messages ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CommandsConfig {
    pub native: Option<String>,
    pub text: Option<bool>,
    pub bash: Option<bool>,
    pub config: Option<bool>,
    pub restart: Option<bool>,
    pub allow_from: Option<HashMap<String, Vec<String>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MessagesConfig {
    pub group_chat: Option<GroupChatConfig>,
}

// ─── Slack (placeholder) ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SlackConfig {
    pub bot_token: Option<String>,
    pub app_token: Option<String>,
    pub signing_secret: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Main Config
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub gateway: GatewayConfig,
    pub agents: AgentsConfig,
    pub session: SessionConfig,
    pub tools: ToolsConfig,
    pub channels: ChannelsConfig,
    pub models: ModelsConfig,
    pub commands: Option<CommandsConfig>,
    pub bindings: Option<Vec<Binding>>,
    pub messages: Option<MessagesConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            gateway: GatewayConfig::default(),
            agents: AgentsConfig::default(),
            session: SessionConfig::default(),
            tools: ToolsConfig::default(),
            channels: ChannelsConfig::default(),
            models: ModelsConfig::default(),
            commands: None,
            bindings: None,
            messages: None,
        }
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let config_path = Self::config_path();
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: Config = json5::from_str(&content)
                .map_err(|e| anyhow::anyhow!("Config parse error: {}", e))?;
            info!("Loaded config from {}", config_path.display());
            Ok(config)
        } else {
            info!("No config file found, using defaults");
            Ok(Config::default())
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let config_path = Self::config_path();
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        info!("Saved config to {}", config_path.display());
        Ok(())
    }

    pub fn config_path() -> PathBuf {
        Self::home_dir().join("klaw.json")
    }

    pub fn home_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".klaw")
    }

    pub fn workspace_dir(&self) -> PathBuf {
        if let Some(ref ws) = self.agents.defaults.workspace {
            PathBuf::from(ws)
        } else {
            Self::home_dir().join("workspace")
        }
    }

    pub fn sessions_dir(&self, agent_id: &str) -> PathBuf {
        Self::home_dir()
            .join("agents")
            .join(agent_id)
            .join("sessions")
    }
}

// ─── Gateway ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GatewayConfig {
    pub port: u16,
    pub host: String,
    pub token: Option<String>,
    pub verbose: bool,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            port: 19789,
            host: "127.0.0.1".to_string(),
            token: None,
            verbose: false,
        }
    }
}

// ─── Agents ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentsConfig {
    pub default: String,
    pub defaults: AgentDefaults,
    pub list: Vec<AgentEntry>,
}

impl Default for AgentsConfig {
    fn default() -> Self {
        Self {
            default: "default".to_string(),
            defaults: AgentDefaults::default(),
            list: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentDefaults {
    pub model: Option<String>,
    pub provider: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub workspace: Option<String>,
    pub bootstrap_max_chars: usize,
    pub bootstrap_total_max_chars: usize,
    pub failover: Option<Vec<String>>,
    pub api_keys: Option<Vec<String>>,
    pub retry_count: Option<u32>,
    pub retry_delay_ms: Option<u64>,
    // New fields
    pub image_model: Option<ModelSpec>,
    pub thinking_default: Option<String>,
    pub verbose_default: Option<String>,
    pub elevated_default: Option<String>,
    pub timeout_seconds: Option<u64>,
    pub media_max_mb: Option<u32>,
    pub context_tokens: Option<u64>,
    pub max_concurrent: Option<u32>,
    pub user_timezone: Option<String>,
    pub time_format: Option<String>,
    pub skip_bootstrap: Option<bool>,
    pub repo_root: Option<String>,
    pub block_streaming_default: Option<String>,
    pub typing_mode: Option<String>,
    pub typing_interval_seconds: Option<u32>,
    pub human_delay: Option<HumanDelayConfig>,
}

impl Default for AgentDefaults {
    fn default() -> Self {
        Self {
            model: None,
            provider: None,
            api_key: None,
            base_url: None,
            workspace: None,
            bootstrap_max_chars: 20000,
            bootstrap_total_max_chars: 150000,
            failover: None,
            api_keys: None,
            retry_count: None,
            retry_delay_ms: None,
            image_model: None,
            thinking_default: None,
            verbose_default: None,
            elevated_default: None,
            timeout_seconds: None,
            media_max_mb: None,
            context_tokens: None,
            max_concurrent: None,
            user_timezone: None,
            time_format: None,
            skip_bootstrap: None,
            repo_root: None,
            block_streaming_default: None,
            typing_mode: None,
            typing_interval_seconds: None,
            human_delay: None,
        }
    }
}

// ─── Agent Entry ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEntry {
    pub id: String,
    pub name: Option<String>,
    pub default: Option<bool>,
    pub model: Option<ModelSpec>,
    pub workspace: Option<String>,
    pub agent_dir: Option<String>,
    pub identity: Option<AgentIdentity>,
    pub heartbeat: Option<HeartbeatConfig>,
    pub sandbox: Option<SandboxConfig>,
    pub tools: Option<ToolsConfig>,
    pub params: Option<serde_json::Value>,
    pub group_chat: Option<GroupChatConfig>,
    pub subagents: Option<SubagentsConfig>,
    pub human_delay: Option<HumanDelayConfig>,
}

// ─── Session ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionConfig {
    pub dm_scope: DmScope,
    pub main_key: String,
    pub scope: Option<String>,
    pub identity_links: Option<HashMap<String, Vec<String>>>,
    pub reset: Option<SessionResetConfig>,
    pub reset_by_type: Option<HashMap<String, SessionResetConfig>>,
    pub reset_triggers: Option<Vec<String>>,
    pub maintenance: Option<SessionMaintenanceConfig>,
    pub thread_bindings: Option<ThreadBindingsConfig>,
    pub agent_to_agent: Option<AgentToAgentConfig>,
    pub send_policy: Option<serde_json::Value>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            dm_scope: DmScope::Main,
            main_key: "main".to_string(),
            scope: None,
            identity_links: None,
            reset: None,
            reset_by_type: None,
            reset_triggers: None,
            maintenance: None,
            thread_bindings: None,
            agent_to_agent: None,
            send_policy: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum DmScope {
    Main,
    PerPeer,
    PerChannelPeer,
    PerAccountChannelPeer,
}

// ─── Tools ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolsConfig {
    pub allow: Option<Vec<String>>,
    pub deny: Option<Vec<String>>,
    pub profile: Option<String>,
    pub by_provider: Option<HashMap<String, ToolsProviderOverride>>,
    pub loop_detection: Option<LoopDetectionConfig>,
    pub elevated: Option<ElevatedConfig>,
    pub web: Option<WebToolsConfig>,
    pub exec: Option<ExecToolsConfig>,
    pub sessions_visibility: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsProviderOverride {
    pub allow: Option<Vec<String>>,
    pub deny: Option<Vec<String>>,
    pub profile: Option<String>,
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            allow: None,
            deny: None,
            profile: None,
            by_provider: None,
            loop_detection: None,
            elevated: None,
            web: None,
            exec: None,
            sessions_visibility: None,
        }
    }
}

// ─── Channels ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ChannelsConfig {
    pub defaults: Option<ChannelDefaults>,
    pub model_by_channel: Option<HashMap<String, HashMap<String, String>>>,
    pub telegram: Option<TelegramConfig>,
    pub discord: Option<DiscordConfig>,
    pub whatsapp: Option<WhatsAppConfig>,
    pub webchat: Option<WebChatConfig>,
    pub slack: Option<SlackConfig>,
    pub signal: Option<serde_json::Value>,
    pub irc: Option<serde_json::Value>,
    pub googlechat: Option<serde_json::Value>,
    pub bluebubbles: Option<serde_json::Value>,
    pub imessage: Option<serde_json::Value>,
    pub msteams: Option<serde_json::Value>,
    pub mattermost: Option<serde_json::Value>,
}

impl Default for ChannelsConfig {
    fn default() -> Self {
        Self {
            defaults: None,
            model_by_channel: None,
            telegram: None,
            discord: None,
            whatsapp: None,
            webchat: Some(WebChatConfig::default()),
            slack: None,
            signal: None,
            irc: None,
            googlechat: None,
            bluebubbles: None,
            imessage: None,
            msteams: None,
            mattermost: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub webhook_url: Option<String>,
    #[serde(default = "default_dm_policy")]
    pub dm_policy: String,
    pub allow_from: Option<Vec<String>>,
    pub groups: Option<HashMap<String, serde_json::Value>>,
    pub custom_commands: Option<Vec<serde_json::Value>>,
    pub history_limit: Option<u32>,
    pub reply_to_mode: Option<String>,
    pub link_preview: Option<bool>,
    pub streaming: Option<String>,
    pub actions: Option<serde_json::Value>,
    pub reaction_notifications: Option<String>,
    pub media_max_mb: Option<u32>,
    pub retry: Option<RetryConfig>,
    pub proxy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    pub bot_token: String,
    #[serde(default = "default_dm_policy")]
    pub dm_policy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppConfig {
    #[serde(default = "default_dm_policy")]
    pub dm_policy: String,
    pub allow_from: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WebChatConfig {
    pub enabled: bool,
}

impl Default for WebChatConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

fn default_dm_policy() -> String {
    "pairing".to_string()
}

// ─── Models ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelsConfig {
    pub aliases: HashMap<String, String>,
}

impl Default for ModelsConfig {
    fn default() -> Self {
        Self {
            aliases: HashMap::new(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.gateway.port, 19789);
        assert_eq!(config.gateway.host, "127.0.0.1");
        assert_eq!(config.session.dm_scope, DmScope::Main);
    }

    #[test]
    fn test_json5_parse() {
        let json5 = r#"{
            // This is a comment
            gateway: {
                port: 9000,
                host: "0.0.0.0",
            },
        }"#;
        let config: Config = json5::from_str(json5).unwrap();
        assert_eq!(config.gateway.port, 9000);
    }

    #[test]
    fn test_session_key() {
        use crate::types::SessionKey;
        let key = SessionKey::main("default");
        assert_eq!(key.0, "agent:default:main");

        let key = SessionKey::group("default", "telegram", "12345");
        assert_eq!(key.0, "agent:default:telegram:12345");
    }

    #[test]
    fn test_model_spec_simple() {
        let json = r#""gpt-4""#;
        let spec: ModelSpec = serde_json::from_str(json).unwrap();
        matches!(spec, ModelSpec::Simple(s) if s == "gpt-4");
    }

    #[test]
    fn test_model_spec_with_fallbacks() {
        let json = r#"{"primary":"gpt-4","fallbacks":["gpt-3.5"]}"#;
        let spec: ModelSpec = serde_json::from_str(json).unwrap();
        matches!(spec, ModelSpec::WithFallbacks { .. });
    }

    #[test]
    fn test_expanded_config_parse() {
        let json5 = r#"{
            commands: { native: "slash", text: true },
            bindings: [{ agent_id: "main", channel: "telegram" }],
            messages: { group_chat: { history_limit: 50 } },
            session: {
                dm_scope: "main",
                main_key: "main",
                reset: { mode: "daily", at_hour: 3 },
                maintenance: { mode: "auto", max_entries: 1000 },
            },
            tools: {
                loop_detection: { enabled: true, warning_threshold: 5 },
                web: { search: { enabled: true, max_results: 10 } },
            },
        }"#;
        let config: Config = json5::from_str(json5).unwrap();
        assert!(config.commands.is_some());
        assert!(config.bindings.is_some());
        assert!(config.session.reset.is_some());
        assert!(config.tools.loop_detection.is_some());
    }
}
