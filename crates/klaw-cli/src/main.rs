use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "klaw",
    version,
    about = "🦀 Klaw — Multi-channel AI gateway",
    long_about = "Klaw is a self-hosted gateway that connects your chat apps to AI agents.\nBuilt in Rust for speed, reliability, and zero dependencies."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start, stop, or check the gateway daemon
    Gateway {
        #[command(subcommand)]
        action: GatewayAction,
    },
    /// Check gateway health
    Health,
    /// Show full status
    Status,
    /// Send a message to an agent
    Agent {
        /// Message to send
        #[arg(short, long)]
        message: String,
        /// Thinking level (off, low, high)
        #[arg(short, long, default_value = "off")]
        thinking: String,
    },
    /// Send a message to a channel
    Message {
        #[command(subcommand)]
        action: MessageAction,
    },
    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// List and manage models
    Models,
    /// Manage OAuth authentication (login, logout, status)
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
    /// Run diagnostics
    Doctor,
    /// Quick test: send a message to LLM and get response
    Test {
        /// Message to send
        #[arg(short, long)]
        message: String,
        /// Provider (anthropic, openai, ollama)
        #[arg(short, long, default_value = "anthropic")]
        provider: String,
        /// Model name
        #[arg(long)]
        model: Option<String>,
        /// API key (or set ANTHROPIC_API_KEY / OPENAI_API_KEY env var)
        #[arg(long)]
        api_key: Option<String>,
    },
    /// Show version info
    Version,

    // ── New commands ──

    /// Manage agents
    Agents {
        #[command(subcommand)]
        action: AgentsAction,
    },
    /// Manage sessions
    Sessions {
        #[command(subcommand)]
        action: SessionsAction,
    },
    /// Manage cron jobs
    Cron {
        #[command(subcommand)]
        action: CronAction,
    },
    /// Manage channels
    Channels {
        #[command(subcommand)]
        action: ChannelsAction,
    },
    /// Manage paired devices
    Devices {
        #[command(subcommand)]
        action: DevicesAction,
    },
    /// Show pairing status
    Pairing,
    /// Search and read memory files
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },
    /// Manage plugins
    Plugins {
        #[command(subcommand)]
        action: PluginsAction,
    },
    /// Manage skills
    Skills {
        #[command(subcommand)]
        action: SkillsAction,
    },
    /// Show gateway logs
    Logs {
        /// Number of lines to show
        #[arg(short = 'n', long, default_value = "50")]
        lines: usize,
    },
    /// Interactive configuration wizard
    Configure,
    /// Factory reset (deletes all data)
    Reset {
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
    /// Generate shell completions
    Completion {
        /// Shell type
        shell: String,
    },
}

#[derive(Subcommand)]
enum GatewayAction {
    /// Start the gateway server
    Start {
        /// Port to listen on
        #[arg(short, long)]
        port: Option<u16>,
        /// Enable verbose logging
        #[arg(short, long)]
        verbose: bool,
    },
    /// Stop the running gateway
    Stop,
    /// Show gateway status
    Status,
    /// Restart the gateway
    Restart,
}

#[derive(Subcommand)]
enum MessageAction {
    /// Send a message
    Send {
        /// Target (phone number, username, channel ID)
        #[arg(long)]
        to: String,
        /// Message text
        #[arg(long)]
        message: String,
        /// Channel (telegram, discord, whatsapp, etc.)
        #[arg(long)]
        channel: Option<String>,
    },
}

#[derive(Subcommand)]
enum AuthAction {
    /// Login to an OAuth provider (Google, Qwen, OpenAI Codex, etc.)
    Login {
        /// Provider name (google-antigravity, qwen-portal, openai-codex, etc.)
        provider: String,
    },
    /// Logout from an OAuth provider
    Logout {
        /// Provider name
        provider: String,
    },
    /// Show OAuth token status
    Status,
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Get a config value
    Get {
        /// Config key (dot-separated, e.g., "gateway.port")
        key: Option<String>,
    },
    /// Set a config value
    Set {
        /// Config key
        key: String,
        /// Config value
        value: String,
    },
}

// ── New subcommand enums ──

#[derive(Subcommand)]
enum AgentsAction {
    /// List configured agents
    List,
    /// Add a new agent
    Add {
        /// Agent ID
        id: String,
    },
}

#[derive(Subcommand)]
enum SessionsAction {
    /// List active sessions
    List,
    /// Inspect a session (show details + recent messages)
    Inspect {
        /// Session key
        key: String,
    },
    /// Reset a session (delete transcript)
    Reset {
        /// Session key
        key: String,
    },
    /// Send a message to a session
    Send {
        /// Session key
        key: String,
        /// Message text
        message: String,
    },
}

#[derive(Subcommand)]
enum CronAction {
    /// List cron jobs
    List,
    /// Add a cron job
    Add {
        /// Cron schedule (e.g., "*/5 * * * *")
        schedule: String,
        /// Task to run
        task: String,
    },
    /// Remove a cron job
    Remove {
        /// Job ID
        id: String,
    },
}

#[derive(Subcommand)]
enum ChannelsAction {
    /// List configured channels
    List,
    /// Check channel connectivity
    Status,
    /// Setup channel credentials
    Login {
        /// Channel name (telegram, discord, whatsapp)
        channel: String,
    },
}

#[derive(Subcommand)]
enum DevicesAction {
    /// List paired devices
    List,
    /// Approve a device
    Approve {
        /// Device ID
        id: String,
    },
    /// Reject a device
    Reject {
        /// Device ID
        id: String,
    },
}

#[derive(Subcommand)]
enum MemoryAction {
    /// Search memory files
    Search {
        /// Search query
        query: String,
    },
    /// Read a memory file
    Get {
        /// File path (relative to workspace)
        path: String,
    },
}

#[derive(Subcommand)]
enum PluginsAction {
    /// List installed plugins
    List,
    /// Install a plugin
    Install {
        /// Plugin name
        name: String,
    },
}

#[derive(Subcommand)]
enum SkillsAction {
    /// List available skills
    List,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Gateway { action } => match action {
            GatewayAction::Start { port, verbose } => {
                let mut config = klaw_core::Config::load()?;
                if let Some(p) = port {
                    config.gateway.port = p;
                }
                if verbose {
                    config.gateway.verbose = true;
                }
                println!("🦀 Klaw Gateway v{}", env!("CARGO_PKG_VERSION"));
                println!("   Starting on {}:{}", config.gateway.host, config.gateway.port);
                println!();
                klaw_gateway::start_gateway(config).await?;
            }
            GatewayAction::Stop => {
                println!("🛑 Stopping gateway...");
                println!("   Not yet implemented — kill the process manually for now");
            }
            GatewayAction::Status => {
                println!("📊 Gateway Status");
                let config = klaw_core::Config::load()?;
                println!("   Config: {}", klaw_core::Config::config_path().display());
                println!("   Port: {}", config.gateway.port);
                println!("   Model: {}", config.agents.defaults.model.as_deref().unwrap_or("not set"));
            }
            GatewayAction::Restart => {
                println!("🔄 Restarting gateway...");
                println!("   Not yet implemented");
            }
        },
        Commands::Health => {
            println!("🏥 Health Check");
            println!("   Status: ok");
            println!("   Version: {}", env!("CARGO_PKG_VERSION"));
        }
        Commands::Status => {
            let config = klaw_core::Config::load()?;
            println!("🦀 Klaw v{}", env!("CARGO_PKG_VERSION"));
            println!();
            println!("Gateway:");
            println!("   Host: {}:{}", config.gateway.host, config.gateway.port);
            println!("   Auth: {}", if config.gateway.token.is_some() { "enabled" } else { "disabled" });
            println!();
            println!("Agent:");
            println!("   Default model: {}", config.agents.defaults.model.as_deref().unwrap_or("not configured"));
            println!("   Workspace: {}", config.workspace_dir().display());
            println!();
            println!("Channels:");
            println!("   WebChat: {}", if config.channels.webchat.is_some() { "✅" } else { "❌" });
            println!("   Telegram: {}", if config.channels.telegram.is_some() { "✅" } else { "❌" });
            println!("   Discord: {}", if config.channels.discord.is_some() { "✅" } else { "❌" });
            println!("   WhatsApp: {}", if config.channels.whatsapp.is_some() { "✅" } else { "❌" });
        }
        Commands::Agent { message, thinking } => {
            println!("🤖 Sending to agent: {}", message);
            println!("   Thinking: {}", thinking);
            println!("   Not yet implemented — gateway must be running");
        }
        Commands::Message { action } => match action {
            MessageAction::Send { to, message, channel } => {
                println!("📤 Sending message");
                println!("   To: {}", to);
                println!("   Channel: {}", channel.as_deref().unwrap_or("auto"));
                println!("   Message: {}", message);
            }
        },
        Commands::Config { action } => match action {
            ConfigAction::Get { key } => {
                let config = klaw_core::Config::load()?;
                match key {
                    Some(k) => {
                        let json = serde_json::to_value(&config)?;
                        let parts: Vec<&str> = k.split('.').collect();
                        let mut current = &json;
                        for part in &parts {
                            current = current.get(part).unwrap_or(&serde_json::Value::Null);
                        }
                        println!("{}", serde_json::to_string_pretty(current)?);
                    }
                    None => {
                        println!("{}", serde_json::to_string_pretty(&config)?);
                    }
                }
            }
            ConfigAction::Set { key, value } => {
                println!("Setting {} = {}", key, value);
            }
        },
        Commands::Models => {
            let config = klaw_core::Config::load()?;
            println!("🧠 Models");
            println!("   Default: {}", config.agents.defaults.model.as_deref().unwrap_or("not set"));

            if !config.models.aliases.is_empty() {
                println!("\nAliases:");
                for (alias, model) in &config.models.aliases {
                    println!("   {} → {}", alias, model);
                }
            }

            println!("\n📦 Available Providers ({}):", klaw_agent::list_providers().len());
            for (id, name, env_key) in klaw_agent::list_providers() {
                let has_key = std::env::var(&env_key).is_ok();
                let status = if has_key { "✅" } else { "  " };
                println!("   {} {:<22} {} ({})", status, id, name, env_key);
            }
        }
        Commands::Auth { action } => match action {
            AuthAction::Login { provider } => {
                let oauth_configs = klaw_agent::providers::oauth::oauth_providers();
                match oauth_configs.get(&provider) {
                    Some(config) => {
                        if config.device_code_url.is_some() {
                            println!("🔐 Starting device code flow for {}...", config.name);
                            if config.client_id.is_empty() {
                                println!("⚠️  No client_id configured for {}.", provider);
                                println!("   Set it in ~/.klaw/klaw.json under models.oauth.{}.client_id", provider);
                                println!("   Or use the provider's CLI tool directly.");
                            } else {
                                match klaw_agent::providers::oauth::request_device_code(config).await {
                                    Ok(resp) => {
                                        println!("\n📱 Go to: {}", resp.verification_uri);
                                        println!("   Enter code: {}\n", resp.user_code);
                                        if let Some(ref url) = resp.verification_uri_complete {
                                            println!("   Or open: {}\n", url);
                                        }
                                        println!("Waiting for authorization...");

                                        match klaw_agent::providers::oauth::poll_device_token(
                                            config, &resp.device_code, resp.interval.unwrap_or(5)
                                        ).await {
                                            Ok(token) => {
                                                let mut store = klaw_agent::providers::oauth::TokenStore::new();
                                                store.set(&provider, token);
                                                println!("✅ Logged in to {}!", config.name);
                                            }
                                            Err(e) => println!("❌ Login failed: {}", e),
                                        }
                                    }
                                    Err(e) => println!("❌ Failed to start device flow: {}", e),
                                }
                            }
                        } else {
                            println!("🔐 {} uses authorization code flow.", config.name);
                            println!("   Run `klaw gateway start` first, then visit:");
                            println!("   http://localhost:19789/auth/{}", provider);
                        }
                    }
                    None => {
                        println!("❌ Unknown OAuth provider: {}", provider);
                        println!("\nAvailable OAuth providers:");
                        for (id, cfg) in &oauth_configs {
                            println!("   {} — {}", id, cfg.name);
                        }
                    }
                }
            }
            AuthAction::Logout { provider } => {
                let mut store = klaw_agent::providers::oauth::TokenStore::new();
                store.remove(&provider);
                println!("🗑️  Removed token for {}", provider);
            }
            AuthAction::Status => {
                let store = klaw_agent::providers::oauth::TokenStore::new();
                let tokens = store.list();
                if tokens.is_empty() {
                    println!("🔐 No OAuth tokens stored.");
                } else {
                    println!("🔐 OAuth tokens:");
                    for provider in &tokens {
                        let valid = store.has_valid_token(provider);
                        println!("   {} {}", if valid { "✅" } else { "⚠️" }, provider);
                    }
                }

                println!("\nAvailable OAuth providers:");
                for (id, cfg) in klaw_agent::providers::oauth::oauth_providers() {
                    let flow = if cfg.device_code_url.is_some() { "device-code" } else { "auth-code" };
                    println!("   {} — {} ({})", id, cfg.name, flow);
                }
            }
        },
        Commands::Doctor => {
            println!("🩺 Klaw Doctor");
            println!();
            println!("Checking...");
            
            let config_path = klaw_core::Config::config_path();
            println!("   Config file: {} {}", config_path.display(),
                if config_path.exists() { "✅" } else { "⚠️ not found (using defaults)" });
            
            let config = klaw_core::Config::load()?;
            let ws = config.workspace_dir();
            println!("   Workspace: {} {}", ws.display(),
                if ws.exists() { "✅" } else { "⚠️ not created yet" });
            
            println!("   Model: {}", match &config.agents.defaults.model {
                Some(m) => format!("{} ✅", m),
                None => "not configured ⚠️".to_string(),
            });
            
            println!();
            println!("Done! Run `klaw gateway start` to start the gateway.");
        }
        Commands::Test { message, provider, model, api_key } => {
            let config = klaw_core::Config::load()?;

            let key = api_key
                .or_else(|| config.agents.defaults.api_key.clone())
                .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                .expect("No API key found. Set --api-key, ANTHROPIC_API_KEY, or OPENAI_API_KEY");

            let model_name = model
                .or_else(|| config.agents.defaults.model.clone())
                .unwrap_or_else(|| match provider.as_str() {
                    "anthropic" => "claude-sonnet-4-20250514".to_string(),
                    _ => "gpt-4".to_string(),
                });

            println!("🧪 Test: {} → {} ({})", provider, model_name, message);
            println!();

            let provider_model = format!("{}/{}", provider, model_name);
            let (llm, _) = klaw_agent::create_provider(
                &provider_model,
                Some(&key),
                config.agents.defaults.base_url.as_deref(),
                &std::collections::HashMap::new(),
            )?;

            let request = klaw_agent::provider::ChatRequest {
                model: model_name,
                messages: vec![
                    klaw_core::types::Message::system("You are a helpful assistant. Be concise."),
                    klaw_core::types::Message::user(&message),
                ],
                tools: None,
                temperature: None,
                max_tokens: Some(1024),
                stream: false,
                thinking: None,
            };

            let response = llm.chat(request).await?;

            println!("📤 Response:");
            println!("{}", response.content.unwrap_or("(empty)".to_string()));
            println!();
            println!("📊 Usage: {} input, {} output, {} total tokens",
                response.usage.input_tokens,
                response.usage.output_tokens,
                response.usage.total_tokens);
        }
        Commands::Version => {
            println!("klaw v{}", env!("CARGO_PKG_VERSION"));
        }

        // ── New command handlers ──

        Commands::Agents { action } => match action {
            AgentsAction::List => {
                let config = klaw_core::Config::load()?;
                println!("🤖 Agents");
                println!("   Default: {}", config.agents.default);
                println!();
                if config.agents.list.is_empty() {
                    // Show at least the default agent
                    println!("   {} (default)", config.agents.default);
                    println!("     Model: {}", config.agents.defaults.model.as_deref().unwrap_or("not set"));
                    println!("     Workspace: {}", config.workspace_dir().display());
                    // Also scan agents dir for any agents created on disk
                    let agents_dir = klaw_core::Config::home_dir().join("agents");
                    if agents_dir.exists() {
                        if let Ok(entries) = std::fs::read_dir(&agents_dir) {
                            for entry in entries.flatten() {
                                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                                    let name = entry.file_name().to_string_lossy().to_string();
                                    if name != config.agents.default {
                                        println!("\n   {}", name);
                                        println!("     Dir: {}", entry.path().display());
                                    }
                                }
                            }
                        }
                    }
                } else {
                    for agent in &config.agents.list {
                        let is_default = agent.id == config.agents.default;
                        println!("   {}{}", agent.id, if is_default { " (default)" } else { "" });
                        let model_str = match &agent.model {
                            Some(klaw_core::config::ModelSpec::Simple(s)) => s.as_str(),
                            Some(klaw_core::config::ModelSpec::WithFallbacks { primary, .. }) => primary.as_str(),
                            None => "(inherit default)",
                        };
                        println!("     Model: {}", model_str);
                        println!("     Workspace: {}", agent.workspace.as_deref().unwrap_or("(inherit default)"));
                        println!();
                    }
                }
            }
            AgentsAction::Add { id } => {
                let config = klaw_core::Config::load()?;
                let agent_dir = klaw_core::Config::home_dir().join("agents").join(&id);
                let workspace_dir = config.workspace_dir().join("agents").join(&id);

                if agent_dir.exists() {
                    println!("⚠️  Agent '{}' already exists at {}", id, agent_dir.display());
                } else {
                    std::fs::create_dir_all(&agent_dir)?;
                    std::fs::create_dir_all(agent_dir.join("sessions"))?;
                    std::fs::create_dir_all(&workspace_dir)?;
                    println!("✅ Created agent '{}'", id);
                    println!("   Agent dir: {}", agent_dir.display());
                    println!("   Workspace: {}", workspace_dir.display());
                    println!();
                    println!("Add it to klaw.json agents.list to configure model/settings.");
                }
            }
        },

        Commands::Sessions { action } => match action {
            SessionsAction::List => {
                let config = klaw_core::Config::load()?;
                let agents_dir = klaw_core::Config::home_dir().join("agents");
                println!("📋 Sessions");
                println!();
                let mut found = false;

                // Scan all agent dirs for sessions
                let agent_ids: Vec<String> = if config.agents.list.is_empty() {
                    vec![config.agents.default.clone()]
                } else {
                    config.agents.list.iter().map(|a| a.id.clone()).collect()
                };

                for agent_id in &agent_ids {
                    let sessions_dir = agents_dir.join(agent_id).join("sessions");
                    if !sessions_dir.exists() {
                        continue;
                    }
                    if let Ok(entries) = std::fs::read_dir(&sessions_dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.extension().map(|e| e == "json").unwrap_or(false) {
                                let name = path.file_stem().unwrap_or_default().to_string_lossy();
                                let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                                println!("   agent:{agent_id}:{name}  ({} bytes)", size);
                                found = true;
                            }
                        }
                    }
                }

                // Also check for a sessions.json in home dir
                let sessions_file = klaw_core::Config::home_dir().join("sessions.json");
                if sessions_file.exists() {
                    let content = std::fs::read_to_string(&sessions_file)?;
                    if let Ok(sessions) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(obj) = sessions.as_object() {
                            for (key, val) in obj {
                                let msg_count = val.get("messages")
                                    .and_then(|m| m.as_array())
                                    .map(|a| a.len())
                                    .unwrap_or(0);
                                println!("   {}  ({} messages)", key, msg_count);
                                found = true;
                            }
                        }
                    }
                }

                if !found {
                    println!("   No sessions found.");
                }
            }
            SessionsAction::Inspect { key } => {
                let config = klaw_core::Config::load()?;
                // Try agent-based session path first
                let parts: Vec<&str> = key.split(':').collect();
                let session_path = if parts.len() >= 2 {
                    config.sessions_dir(parts[1]).join(format!("{}.json", parts.last().unwrap_or(&"main")))
                } else {
                    klaw_core::Config::home_dir().join("agents").join(&config.agents.default).join("sessions").join(format!("{}.json", key))
                };

                // Also check sessions.json
                let sessions_file = klaw_core::Config::home_dir().join("sessions.json");

                if session_path.exists() {
                    let content = std::fs::read_to_string(&session_path)?;
                    let val: serde_json::Value = serde_json::from_str(&content)?;
                    println!("🔍 Session: {}", key);
                    println!("   File: {}", session_path.display());
                    if let Some(messages) = val.get("messages").and_then(|m| m.as_array()) {
                        println!("   Messages: {}", messages.len());
                        println!();
                        // Show last 5 messages
                        let start = if messages.len() > 5 { messages.len() - 5 } else { 0 };
                        for msg in &messages[start..] {
                            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("?");
                            let content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");
                            let preview = if content.len() > 120 { &content[..120] } else { content };
                            println!("   [{}] {}", role, preview);
                        }
                    }
                } else if sessions_file.exists() {
                    let content = std::fs::read_to_string(&sessions_file)?;
                    let sessions: serde_json::Value = serde_json::from_str(&content)?;
                    if let Some(session) = sessions.get(&key) {
                        println!("🔍 Session: {}", key);
                        if let Some(messages) = session.get("messages").and_then(|m| m.as_array()) {
                            println!("   Messages: {}", messages.len());
                            println!();
                            let start = if messages.len() > 5 { messages.len() - 5 } else { 0 };
                            for msg in &messages[start..] {
                                let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("?");
                                let content_str = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");
                                let preview = if content_str.len() > 120 { &content_str[..120] } else { content_str };
                                println!("   [{}] {}", role, preview);
                            }
                        }
                    } else {
                        println!("❌ Session '{}' not found", key);
                    }
                } else {
                    println!("❌ Session '{}' not found", key);
                }
            }
            SessionsAction::Reset { key } => {
                let config = klaw_core::Config::load()?;
                let parts: Vec<&str> = key.split(':').collect();
                let session_path = if parts.len() >= 2 {
                    config.sessions_dir(parts[1]).join(format!("{}.json", parts.last().unwrap_or(&"main")))
                } else {
                    klaw_core::Config::home_dir().join("agents").join(&config.agents.default).join("sessions").join(format!("{}.json", key))
                };

                if session_path.exists() {
                    std::fs::remove_file(&session_path)?;
                    println!("🗑️  Deleted session: {}", key);
                } else {
                    // Try sessions.json
                    let sessions_file = klaw_core::Config::home_dir().join("sessions.json");
                    if sessions_file.exists() {
                        let content = std::fs::read_to_string(&sessions_file)?;
                        let mut sessions: serde_json::Value = serde_json::from_str(&content)?;
                        if let Some(obj) = sessions.as_object_mut() {
                            if obj.remove(&key).is_some() {
                                std::fs::write(&sessions_file, serde_json::to_string_pretty(&sessions)?)?;
                                println!("🗑️  Deleted session: {}", key);
                            } else {
                                println!("❌ Session '{}' not found", key);
                            }
                        }
                    } else {
                        println!("❌ Session '{}' not found", key);
                    }
                }
            }
            SessionsAction::Send { key, message } => {
                println!("📤 Sending to session '{}':", key);
                println!("   Message: {}", message);
                println!("   ⚠️  Direct session messaging requires a running gateway.");
                println!("   Use `klaw gateway start` first, then POST to the API.");
            }
        },

        Commands::Cron { action } => match action {
            CronAction::List => {
                let cron_file = klaw_core::Config::home_dir().join("cron.json");
                println!("⏰ Cron Jobs");
                println!();
                if cron_file.exists() {
                    let content = std::fs::read_to_string(&cron_file)?;
                    let jobs: serde_json::Value = serde_json::from_str(&content)?;
                    if let Some(arr) = jobs.as_array() {
                        if arr.is_empty() {
                            println!("   No cron jobs configured.");
                        } else {
                            for (i, job) in arr.iter().enumerate() {
                                let id = job.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                                let schedule = job.get("schedule").and_then(|v| v.as_str()).unwrap_or("?");
                                let task = job.get("task").and_then(|v| v.as_str()).unwrap_or("?");
                                let enabled = job.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
                                println!("   {}. [{}] {} — {} {}", i + 1, id, schedule, task,
                                    if !enabled { "(disabled)" } else { "" });
                            }
                        }
                    }
                } else {
                    println!("   No cron jobs configured.");
                    println!("   File: {}", cron_file.display());
                }
            }
            CronAction::Add { schedule, task } => {
                let cron_file = klaw_core::Config::home_dir().join("cron.json");
                let mut jobs: Vec<serde_json::Value> = if cron_file.exists() {
                    let content = std::fs::read_to_string(&cron_file)?;
                    serde_json::from_str(&content).unwrap_or_default()
                } else {
                    vec![]
                };

                let id = format!("cron-{}", jobs.len() + 1);
                let job = serde_json::json!({
                    "id": id,
                    "schedule": schedule,
                    "task": task,
                    "enabled": true,
                });
                jobs.push(job);

                if let Some(parent) = cron_file.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&cron_file, serde_json::to_string_pretty(&jobs)?)?;
                println!("✅ Added cron job '{}'", id);
                println!("   Schedule: {}", schedule);
                println!("   Task: {}", task);
            }
            CronAction::Remove { id } => {
                let cron_file = klaw_core::Config::home_dir().join("cron.json");
                if cron_file.exists() {
                    let content = std::fs::read_to_string(&cron_file)?;
                    let mut jobs: Vec<serde_json::Value> = serde_json::from_str(&content).unwrap_or_default();
                    let before = jobs.len();
                    jobs.retain(|j| j.get("id").and_then(|v| v.as_str()) != Some(&id));
                    if jobs.len() < before {
                        std::fs::write(&cron_file, serde_json::to_string_pretty(&jobs)?)?;
                        println!("🗑️  Removed cron job '{}'", id);
                    } else {
                        println!("❌ Cron job '{}' not found", id);
                    }
                } else {
                    println!("❌ No cron jobs configured.");
                }
            }
        },

        Commands::Channels { action } => match action {
            ChannelsAction::List => {
                let config = klaw_core::Config::load()?;
                println!("📡 Channels");
                println!();
                println!("   WebChat:   {}", match &config.channels.webchat {
                    Some(wc) => if wc.enabled { "✅ enabled".to_string() } else { "❌ disabled".to_string() },
                    None => "❌ not configured".to_string(),
                });
                println!("   Telegram:  {}", match &config.channels.telegram {
                    Some(_) => "✅ configured".to_string(),
                    None => "❌ not configured".to_string(),
                });
                println!("   Discord:   {}", match &config.channels.discord {
                    Some(_) => "✅ configured".to_string(),
                    None => "❌ not configured".to_string(),
                });
                println!("   WhatsApp:  {}", match &config.channels.whatsapp {
                    Some(_) => "✅ configured".to_string(),
                    None => "❌ not configured".to_string(),
                });
            }
            ChannelsAction::Status => {
                let config = klaw_core::Config::load()?;
                println!("📡 Channel Status");
                println!();

                // WebChat - always local
                if let Some(wc) = &config.channels.webchat {
                    println!("   WebChat: {}", if wc.enabled { "✅ ready (local)" } else { "❌ disabled" });
                }

                // Telegram - check if token exists
                if let Some(tg) = &config.channels.telegram {
                    let has_token = !tg.bot_token.is_empty();
                    println!("   Telegram: {} (token {})",
                        if has_token { "✅" } else { "⚠️" },
                        if has_token { "set" } else { "missing" });
                } else {
                    println!("   Telegram: ❌ not configured");
                }

                // Discord
                if let Some(dc) = &config.channels.discord {
                    let has_token = !dc.bot_token.is_empty();
                    println!("   Discord:  {} (token {})",
                        if has_token { "✅" } else { "⚠️" },
                        if has_token { "set" } else { "missing" });
                } else {
                    println!("   Discord:  ❌ not configured");
                }

                // WhatsApp
                if config.channels.whatsapp.is_some() {
                    println!("   WhatsApp: ✅ configured");
                } else {
                    println!("   WhatsApp: ❌ not configured");
                }
            }
            ChannelsAction::Login { channel } => {
                println!("🔐 Channel Login: {}", channel);
                match channel.as_str() {
                    "telegram" => {
                        println!("   1. Create a bot via @BotFather on Telegram");
                        println!("   2. Copy the bot token");
                        println!("   3. Set it in klaw.json:");
                        println!("      klaw config set channels.telegram.bot_token YOUR_TOKEN");
                    }
                    "discord" => {
                        println!("   1. Create a bot at https://discord.com/developers/applications");
                        println!("   2. Copy the bot token");
                        println!("   3. Set it in klaw.json:");
                        println!("      klaw config set channels.discord.bot_token YOUR_TOKEN");
                    }
                    "whatsapp" => {
                        println!("   WhatsApp requires the Baileys bridge.");
                        println!("   Run `klaw gateway start` and scan the QR code.");
                    }
                    _ => {
                        println!("   Unknown channel: {}", channel);
                        println!("   Available: telegram, discord, whatsapp, webchat");
                    }
                }
            }
        },

        Commands::Devices { action } => match action {
            DevicesAction::List => {
                let devices_file = klaw_core::Config::home_dir().join("devices.json");
                println!("📱 Paired Devices");
                println!();
                if devices_file.exists() {
                    let content = std::fs::read_to_string(&devices_file)?;
                    let devices: serde_json::Value = serde_json::from_str(&content)?;
                    if let Some(arr) = devices.as_array() {
                        if arr.is_empty() {
                            println!("   No paired devices.");
                        } else {
                            for device in arr {
                                let id = device.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                                let name = device.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
                                let status = device.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
                                let icon = match status {
                                    "approved" => "✅",
                                    "pending" => "⏳",
                                    "rejected" => "❌",
                                    _ => "❓",
                                };
                                println!("   {} {} — {} ({})", icon, id, name, status);
                            }
                        }
                    }
                } else {
                    println!("   No paired devices.");
                }
            }
            DevicesAction::Approve { id } => {
                let devices_file = klaw_core::Config::home_dir().join("devices.json");
                if devices_file.exists() {
                    let content = std::fs::read_to_string(&devices_file)?;
                    let mut devices: Vec<serde_json::Value> = serde_json::from_str(&content)?;
                    let mut found = false;
                    for device in &mut devices {
                        if device.get("id").and_then(|v| v.as_str()) == Some(&id) {
                            device["status"] = serde_json::json!("approved");
                            found = true;
                        }
                    }
                    if found {
                        std::fs::write(&devices_file, serde_json::to_string_pretty(&devices)?)?;
                        println!("✅ Approved device '{}'", id);
                    } else {
                        println!("❌ Device '{}' not found", id);
                    }
                } else {
                    println!("❌ No devices file found. No pending devices.");
                }
            }
            DevicesAction::Reject { id } => {
                let devices_file = klaw_core::Config::home_dir().join("devices.json");
                if devices_file.exists() {
                    let content = std::fs::read_to_string(&devices_file)?;
                    let mut devices: Vec<serde_json::Value> = serde_json::from_str(&content)?;
                    let mut found = false;
                    for device in &mut devices {
                        if device.get("id").and_then(|v| v.as_str()) == Some(&id) {
                            device["status"] = serde_json::json!("rejected");
                            found = true;
                        }
                    }
                    if found {
                        std::fs::write(&devices_file, serde_json::to_string_pretty(&devices)?)?;
                        println!("❌ Rejected device '{}'", id);
                    } else {
                        println!("❌ Device '{}' not found", id);
                    }
                } else {
                    println!("❌ No devices file found. No pending devices.");
                }
            }
        },

        Commands::Pairing => {
            let devices_file = klaw_core::Config::home_dir().join("devices.json");
            println!("🔗 Pairing Status");
            println!();
            if devices_file.exists() {
                let content = std::fs::read_to_string(&devices_file)?;
                let devices: Vec<serde_json::Value> = serde_json::from_str(&content)?;
                let pending: Vec<_> = devices.iter().filter(|d| d.get("status").and_then(|v| v.as_str()) == Some("pending")).collect();
                let approved: Vec<_> = devices.iter().filter(|d| d.get("status").and_then(|v| v.as_str()) == Some("approved")).collect();
                let rejected: Vec<_> = devices.iter().filter(|d| d.get("status").and_then(|v| v.as_str()) == Some("rejected")).collect();
                println!("   Total:    {}", devices.len());
                println!("   Approved: {}", approved.len());
                println!("   Pending:  {}", pending.len());
                println!("   Rejected: {}", rejected.len());
                if !pending.is_empty() {
                    println!();
                    println!("   Pending devices:");
                    for d in &pending {
                        let id = d.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                        let name = d.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
                        println!("     ⏳ {} — {}", id, name);
                    }
                    println!();
                    println!("   Use `klaw devices approve <id>` or `klaw devices reject <id>`");
                }
            } else {
                println!("   No devices paired yet.");
                println!("   Pairing happens when a device connects to the gateway.");
            }
        }

        Commands::Memory { action } => match action {
            MemoryAction::Search { query } => {
                let config = klaw_core::Config::load()?;
                let workspace = config.workspace_dir();
                let memory_dir = workspace.join("memory");
                println!("🔍 Searching memory for: {}", query);
                println!();

                let query_lower = query.to_lowercase();
                let mut results = 0;

                // Search workspace root .md files
                let search_dirs = vec![workspace.clone(), memory_dir];
                for dir in search_dirs {
                    if !dir.exists() { continue; }
                    if let Ok(entries) = std::fs::read_dir(&dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if !path.is_file() { continue; }
                            if let Some(ext) = path.extension() {
                                if ext != "md" && ext != "txt" && ext != "json" { continue; }
                            } else {
                                continue;
                            }
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                let content_lower = content.to_lowercase();
                                if content_lower.contains(&query_lower) {
                                    // Find matching lines
                                    for (i, line) in content.lines().enumerate() {
                                        if line.to_lowercase().contains(&query_lower) {
                                            let relative = path.strip_prefix(&config.workspace_dir()).unwrap_or(&path);
                                            println!("   {}:{} — {}", relative.display(), i + 1, line.trim());
                                            results += 1;
                                            if results >= 20 { break; }
                                        }
                                    }
                                }
                            }
                            if results >= 20 { break; }
                        }
                    }
                    if results >= 20 { break; }
                }

                if results == 0 {
                    println!("   No results found.");
                } else {
                    println!("\n   {} matches found.", results);
                }
            }
            MemoryAction::Get { path } => {
                let config = klaw_core::Config::load()?;
                let full_path = config.workspace_dir().join(&path);
                if full_path.exists() {
                    let content = std::fs::read_to_string(&full_path)?;
                    println!("{}", content);
                } else {
                    // Try memory subdir
                    let memory_path = config.workspace_dir().join("memory").join(&path);
                    if memory_path.exists() {
                        let content = std::fs::read_to_string(&memory_path)?;
                        println!("{}", content);
                    } else {
                        println!("❌ File not found: {}", path);
                        println!("   Searched:");
                        println!("     {}", full_path.display());
                        println!("     {}", memory_path.display());
                    }
                }
            }
        },

        Commands::Plugins { action } => match action {
            PluginsAction::List => {
                let plugins_dir = klaw_core::Config::home_dir().join("plugins");
                println!("🧩 Plugins");
                println!();
                if plugins_dir.exists() {
                    let mut count = 0;
                    if let Ok(entries) = std::fs::read_dir(&plugins_dir) {
                        for entry in entries.flatten() {
                            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                                let name = entry.file_name().to_string_lossy().to_string();
                                let manifest = entry.path().join("plugin.json");
                                if manifest.exists() {
                                    if let Ok(content) = std::fs::read_to_string(&manifest) {
                                        if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&content) {
                                            let desc = meta.get("description").and_then(|v| v.as_str()).unwrap_or("");
                                            let ver = meta.get("version").and_then(|v| v.as_str()).unwrap_or("?");
                                            println!("   {} v{} — {}", name, ver, desc);
                                            count += 1;
                                            continue;
                                        }
                                    }
                                }
                                println!("   {}", name);
                                count += 1;
                            }
                        }
                    }
                    if count == 0 {
                        println!("   No plugins installed.");
                    }
                } else {
                    println!("   No plugins installed.");
                    println!("   Dir: {}", plugins_dir.display());
                }
            }
            PluginsAction::Install { name } => {
                println!("🧩 Installing plugin: {}", name);
                let plugins_dir = klaw_core::Config::home_dir().join("plugins").join(&name);
                std::fs::create_dir_all(&plugins_dir)?;
                // Create a minimal plugin.json
                let manifest = serde_json::json!({
                    "name": name,
                    "version": "0.0.1",
                    "description": "Placeholder plugin",
                    "installed": true,
                });
                std::fs::write(plugins_dir.join("plugin.json"), serde_json::to_string_pretty(&manifest)?)?;
                println!("   Created placeholder at: {}", plugins_dir.display());
                println!("   ⚠️  Plugin registry not yet implemented — this is a local placeholder.");
            }
        },

        Commands::Skills { action } => match action {
            SkillsAction::List => {
                let config = klaw_core::Config::load()?;
                println!("🎯 Skills");
                println!();

                let mut found = 0;
                // Check workspace/skills
                let skill_dirs = vec![
                    config.workspace_dir().join("skills"),
                    klaw_core::Config::home_dir().join("skills"),
                ];

                for dir in skill_dirs {
                    if !dir.exists() { continue; }
                    if let Ok(entries) = std::fs::read_dir(&dir) {
                        for entry in entries.flatten() {
                            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                                let name = entry.file_name().to_string_lossy().to_string();
                                let skill_md = entry.path().join("SKILL.md");
                                if skill_md.exists() {
                                    // Try to read first line for description
                                    if let Ok(content) = std::fs::read_to_string(&skill_md) {
                                        let desc = content.lines()
                                            .find(|l| !l.starts_with('#') && !l.trim().is_empty())
                                            .unwrap_or("No description");
                                        println!("   {} — {}", name, desc.trim());
                                    } else {
                                        println!("   {}", name);
                                    }
                                } else {
                                    println!("   {} (no SKILL.md)", name);
                                }
                                found += 1;
                            }
                        }
                    }
                }

                if found == 0 {
                    println!("   No skills found.");
                    println!("   Skills are directories with a SKILL.md file.");
                }
            }
        },

        Commands::Logs { lines } => {
            let log_file = klaw_core::Config::home_dir().join("klaw.log");
            if log_file.exists() {
                let content = std::fs::read_to_string(&log_file)?;
                let all_lines: Vec<&str> = content.lines().collect();
                let start = if all_lines.len() > lines { all_lines.len() - lines } else { 0 };
                for line in &all_lines[start..] {
                    println!("{}", line);
                }
            } else {
                println!("📄 No log file found at {}", log_file.display());
                println!("   Logs are created when the gateway runs.");
            }
        }

        Commands::Configure => {
            println!("⚙️  Klaw Configuration Wizard");
            println!();
            println!("   Config file: {}", klaw_core::Config::config_path().display());
            println!();
            println!("   Interactive wizard is not yet implemented.");
            println!("   For now, edit the config file directly or use:");
            println!();
            println!("   klaw config get              — View current config");
            println!("   klaw config set <key> <val>  — Set a value");
            println!();
            println!("   Common settings:");
            println!("     klaw config set agents.defaults.model anthropic/claude-sonnet-4-20250514");
            println!("     klaw config set gateway.port 19789");
            println!("     klaw config set channels.telegram.bot_token YOUR_TOKEN");
        }

        Commands::Reset { force } => {
            let home = klaw_core::Config::home_dir();
            if !force {
                println!("⚠️  This will delete ALL Klaw data:");
                println!("   {}", home.display());
                println!();
                println!("   This includes config, sessions, agents, plugins, and all data.");
                println!();
                println!("   Run with --force to confirm: klaw reset --force");
            } else {
                if home.exists() {
                    // Backup config first
                    let config_path = klaw_core::Config::config_path();
                    let backup = home.join("klaw.json.bak");
                    if config_path.exists() {
                        std::fs::copy(&config_path, &backup).ok();
                        println!("   Backed up config to {}", backup.display());
                    }

                    // Remove sessions, agents data, plugins, but keep config backup
                    let dirs_to_remove = vec!["agents", "sessions", "plugins", "skills"];
                    for dir_name in dirs_to_remove {
                        let dir = home.join(dir_name);
                        if dir.exists() {
                            std::fs::remove_dir_all(&dir)?;
                            println!("   Removed {}/", dir_name);
                        }
                    }

                    // Remove specific files
                    let files_to_remove = vec!["sessions.json", "cron.json", "devices.json", "klaw.log"];
                    for file_name in files_to_remove {
                        let file = home.join(file_name);
                        if file.exists() {
                            std::fs::remove_file(&file)?;
                            println!("   Removed {}", file_name);
                        }
                    }

                    println!();
                    println!("🔄 Reset complete. Config backup saved as klaw.json.bak");
                    println!("   Run `klaw gateway start` to start fresh.");
                } else {
                    println!("   Nothing to reset — {} doesn't exist.", home.display());
                }
            }
        }

        Commands::Completion { shell } => {
            match shell.as_str() {
                "bash" | "zsh" | "fish" | "powershell" | "elvish" => {
                    println!("🐚 Shell completion for: {}", shell);
                    println!();
                    println!("   Shell completion generation coming soon.");
                    println!("   This will use clap_complete to generate {} completions.", shell);
                    println!();
                    println!("   For now, you can use --help on any subcommand:");
                    println!("     klaw --help");
                    println!("     klaw gateway --help");
                    println!("     klaw agents --help");
                }
                _ => {
                    println!("❌ Unknown shell: {}", shell);
                    println!("   Supported: bash, zsh, fish, powershell, elvish");
                }
            }
        }
    }

    Ok(())
}
