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
                // TODO: Send shutdown signal to running gateway
                println!("   Not yet implemented — kill the process manually for now");
            }
            GatewayAction::Status => {
                println!("📊 Gateway Status");
                let config = klaw_core::Config::load()?;
                println!("   Config: {}", klaw_core::Config::config_path().display());
                println!("   Port: {}", config.gateway.port);
                println!("   Model: {}", config.agents.defaults.model.as_deref().unwrap_or("not set"));
                // TODO: Check if gateway is actually running
            }
            GatewayAction::Restart => {
                println!("🔄 Restarting gateway...");
                // TODO: Implement restart
                println!("   Not yet implemented");
            }
        },
        Commands::Health => {
            println!("🏥 Health Check");
            println!("   Status: ok");
            println!("   Version: {}", env!("CARGO_PKG_VERSION"));
            // TODO: Actually check gateway health via HTTP
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
            // TODO: Send message to agent via gateway WS
            println!("   Not yet implemented — gateway must be running");
        }
        Commands::Message { action } => match action {
            MessageAction::Send { to, message, channel } => {
                println!("📤 Sending message");
                println!("   To: {}", to);
                println!("   Channel: {}", channel.as_deref().unwrap_or("auto"));
                println!("   Message: {}", message);
                // TODO: Route through gateway
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
                // TODO: Implement config set
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
            
            // Config
            let config_path = klaw_core::Config::config_path();
            println!("   Config file: {} {}", config_path.display(),
                if config_path.exists() { "✅" } else { "⚠️ not found (using defaults)" });
            
            // Workspace
            let config = klaw_core::Config::load()?;
            let ws = config.workspace_dir();
            println!("   Workspace: {} {}", ws.display(),
                if ws.exists() { "✅" } else { "⚠️ not created yet" });
            
            // Model
            println!("   Model: {}", match &config.agents.defaults.model {
                Some(m) => format!("{} ✅", m),
                None => "not configured ⚠️".to_string(),
            });
            
            println!();
            println!("Done! Run `klaw gateway start` to start the gateway.");
        }
        Commands::Test { message, provider, model, api_key } => {
            let config = klaw_core::Config::load()?;

            // Resolve API key
            let key = api_key
                .or_else(|| config.agents.defaults.api_key.clone())
                .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                .expect("No API key found. Set --api-key, ANTHROPIC_API_KEY, or OPENAI_API_KEY");

            // Resolve model
            let model_name = model
                .or_else(|| config.agents.defaults.model.clone())
                .unwrap_or_else(|| match provider.as_str() {
                    "anthropic" => "claude-sonnet-4-20250514".to_string(),
                    _ => "gpt-4".to_string(),
                });

            println!("🧪 Test: {} → {} ({})", provider, model_name, message);
            println!();

            // Create provider using registry
            let provider_model = format!("{}/{}", provider, model_name);
            let (llm, _) = klaw_agent::create_provider(
                &provider_model,
                Some(&key),
                config.agents.defaults.base_url.as_deref(),
                &std::collections::HashMap::new(),
            )?;

            // Simple non-streaming test
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
    }

    Ok(())
}
