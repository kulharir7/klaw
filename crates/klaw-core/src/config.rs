use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::info;

/// Main Klaw configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub gateway: GatewayConfig,
    pub agents: AgentsConfig,
    pub session: SessionConfig,
    pub tools: ToolsConfig,
    pub channels: ChannelsConfig,
    pub models: ModelsConfig,
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
        }
    }
}

impl Config {
    /// Load config from `~/.klaw/klaw.json` (JSON5 format)
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

    /// Save config to `~/.klaw/klaw.json`
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

    /// Get the config file path
    pub fn config_path() -> PathBuf {
        Self::home_dir().join("klaw.json")
    }

    /// Get the Klaw home directory (`~/.klaw/`)
    pub fn home_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".klaw")
    }

    /// Get the workspace directory
    pub fn workspace_dir(&self) -> PathBuf {
        if let Some(ref ws) = self.agents.defaults.workspace {
            PathBuf::from(ws)
        } else {
            Self::home_dir().join("workspace")
        }
    }

    /// Get the sessions directory for an agent
    pub fn sessions_dir(&self, agent_id: &str) -> PathBuf {
        Self::home_dir()
            .join("agents")
            .join(agent_id)
            .join("sessions")
    }
}

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
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEntry {
    pub id: String,
    pub model: Option<String>,
    pub workspace: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionConfig {
    pub dm_scope: DmScope,
    pub main_key: String,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            dm_scope: DmScope::Main,
            main_key: "main".to_string(),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolsConfig {
    pub allow: Option<Vec<String>>,
    pub deny: Option<Vec<String>>,
    pub profile: Option<String>,
    pub by_provider: Option<std::collections::HashMap<String, ToolsProviderOverride>>,
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
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ChannelsConfig {
    pub telegram: Option<TelegramConfig>,
    pub discord: Option<DiscordConfig>,
    pub whatsapp: Option<WhatsAppConfig>,
    pub webchat: Option<WebChatConfig>,
}

impl Default for ChannelsConfig {
    fn default() -> Self {
        Self {
            telegram: None,
            discord: None,
            whatsapp: None,
            webchat: Some(WebChatConfig::default()),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelsConfig {
    pub aliases: std::collections::HashMap<String, String>,
}

impl Default for ModelsConfig {
    fn default() -> Self {
        Self {
            aliases: std::collections::HashMap::new(),
        }
    }
}

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
}
