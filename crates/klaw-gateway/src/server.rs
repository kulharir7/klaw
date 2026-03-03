use axum::{
    Router,
    body::Bytes,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    http::HeaderMap,
    response::{Html, Json},
    routing::{get, post},
};
use klaw_core::config::Config;
use klaw_core::session::SessionStore;
use klaw_core::types::SessionKey;
use klaw_agent::{AgentConfig, SystemPromptBuilder, run_agent};
use klaw_agent::provider::LlmProvider;
use klaw_tools::{ToolContext, create_default_registry};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Device identity for connected clients/nodes
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceIdentity {
    pub device_id: String,
    pub platform: String,
    pub role: String, // "operator" or "node"
    pub client_id: String,
    pub client_version: String,
    pub caps: Vec<String>,
    pub commands: Vec<String>,
    pub paired: bool,
    pub device_token: Option<String>,
}

/// Paired device store (persisted)
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PairingStore {
    pub devices: HashMap<String, DeviceIdentity>,
    pub pending: HashMap<String, DeviceIdentity>,
}

impl PairingStore {
    pub fn load() -> Self {
        let path = Config::home_dir().join("pairing.json");
        if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|c| serde_json::from_str(&c).ok())
                .unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) {
        let path = Config::home_dir().join("pairing.json");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, serde_json::to_string_pretty(self).unwrap_or_default());
    }

    pub fn is_paired(&self, device_id: &str) -> bool {
        self.devices.contains_key(device_id)
    }

    pub fn approve(&mut self, device_id: &str) -> Option<String> {
        if let Some(mut dev) = self.pending.remove(device_id) {
            let token = uuid::Uuid::new_v4().to_string();
            dev.paired = true;
            dev.device_token = Some(token.clone());
            self.devices.insert(device_id.to_string(), dev);
            self.save();
            Some(token)
        } else {
            None
        }
    }

    pub fn reject(&mut self, device_id: &str) {
        self.pending.remove(device_id);
        self.save();
    }
}

/// Idempotency cache — prevents duplicate side-effecting requests
pub struct IdempotencyCache {
    cache: HashMap<String, (serde_json::Value, Instant)>,
    ttl_secs: u64,
}

impl IdempotencyCache {
    pub fn new(ttl_secs: u64) -> Self {
        Self { cache: HashMap::new(), ttl_secs }
    }

    pub fn get(&mut self, key: &str) -> Option<&serde_json::Value> {
        // Cleanup expired
        let now = Instant::now();
        self.cache.retain(|_, (_, ts)| now.duration_since(*ts).as_secs() < self.ttl_secs);
        self.cache.get(key).map(|(v, _)| v)
    }

    pub fn set(&mut self, key: String, response: serde_json::Value) {
        self.cache.insert(key, (response, Instant::now()));
    }
}

/// Presence info for connected clients
#[derive(Debug, Clone, serde::Serialize)]
pub struct PresenceInfo {
    pub device_id: String,
    pub role: String,
    pub connected_at: i64,
    pub last_seen: i64,
}

/// Shared gateway state
pub struct GatewayState {
    pub config: Config,
    pub session_store: SessionStore,
    pub provider: Box<dyn LlmProvider>,
    pub pairing_store: PairingStore,
    pub idempotency: IdempotencyCache,
    pub presence: HashMap<String, PresenceInfo>,
    pub started_at: Instant,
    pub gateway_token: Option<String>,
}

/// Start the gateway HTTP + WebSocket server
pub async fn start_gateway(config: Config) -> anyhow::Result<()> {
    let addr = format!("{}:{}", config.gateway.host, config.gateway.port);

    // Check lock file (prevent double-start)
    let lock_path = Config::home_dir().join("gateway.lock");
    if lock_path.exists() {
        let pid = std::fs::read_to_string(&lock_path).unwrap_or_default();
        warn!("Lock file exists (PID: {}). If gateway is not running, delete {}", pid.trim(), lock_path.display());
    }
    // Write PID lock
    std::fs::create_dir_all(Config::home_dir())?;
    std::fs::write(&lock_path, std::process::id().to_string())?;

    // Gateway auth token
    let gateway_token = config.gateway.token.clone()
        .or_else(|| std::env::var("KLAW_GATEWAY_TOKEN").ok());

    // Create LLM provider
    let provider: Box<dyn LlmProvider> = create_llm_provider(&config)?;
    info!("LLM provider: {}", provider.name());

    let state = Arc::new(RwLock::new(GatewayState {
        config: config.clone(),
        session_store: SessionStore::new(config.clone()),
        provider,
        pairing_store: PairingStore::load(),
        idempotency: IdempotencyCache::new(300), // 5 min TTL
        presence: HashMap::new(),
        started_at: Instant::now(),
        gateway_token,
    }));

    // Tick event broadcaster (heartbeat keepalive)
    let tick_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
        loop {
            interval.tick().await;
            let state = tick_state.read().await;
            let uptime = state.started_at.elapsed().as_secs();
            drop(state);
            // Tick events sent per-connection in handle_ws_connection
            let _ = uptime; // used in future for health
        }
    });

    let app = Router::new()
        .route("/", get(webchat_handler))
        .route("/health", get({
            let state = state.clone();
            move || health_handler_with_state(state)
        }))
        .route("/__klaw__/canvas/{path:.*}", get(canvas_handler))
        .route("/__klaw__/a2ui/{path:.*}", get(a2ui_handler))
        .route("/webhook", post({
            let state = state.clone();
            move |headers: HeaderMap, body: Bytes| webhook_handler(headers, body, state)
        }))
        .route("/ws", get({
            let state = state.clone();
            move |ws: WebSocketUpgrade| async move {
                ws.on_upgrade(move |socket| handle_ws_connection(socket, state))
            }
        }))
        // OpenAI-compatible API endpoints
        .route("/v1/chat/completions", post({
            let state = state.clone();
            move |headers: HeaderMap, body: Bytes| chat_completions_handler(headers, body, state)
        }))
        .route("/v1/models", get({
            let state = state.clone();
            move || models_handler(state)
        }))
        .route("/v1/tools/invoke", post({
            let state = state.clone();
            move |headers: HeaderMap, body: Bytes| tools_invoke_handler(headers, body, state)
        }))
        // Dashboard API endpoints
        .route("/api/stats", get({
            let state = state.clone();
            move || stats_handler(state)
        }))
        .route("/api/sessions", get({
            let state = state.clone();
            move || sessions_list_handler(state)
        }))
        .route("/api/usage", get({
            let state = state.clone();
            move || usage_handler(state)
        }))
        .route("/api/config", get({
            let state = state.clone();
            move || config_handler(state)
        }));

    info!("🦀 Klaw Gateway v{} starting on {}", env!("CARGO_PKG_VERSION"), addr);
    {
        let s = state.read().await;
        if s.gateway_token.is_some() {
            info!("🔒 Gateway auth: enabled");
        }
    }

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("✅ Gateway listening on http://{}", addr);
    info!("   WebChat:  http://{}", addr);
    info!("   Canvas:   http://{}/__klaw__/canvas/", addr);
    info!("   A2UI:     http://{}/__klaw__/a2ui/", addr);
    info!("   Webhook:  http://{}/webhook", addr);
    info!("   Health:   http://{}/health", addr);

    // Start Telegram channel if configured
    if let Some(ref tg_config) = config.channels.telegram {
        let bot_token = &tg_config.bot_token;
        if !bot_token.is_empty() {
            info!("🤖 Starting Telegram channel...");
            let tg_state = state.clone();
            let token = bot_token.clone();
            tokio::spawn(async move {
                if let Err(e) = run_telegram_loop(token, tg_state).await {
                    tracing::error!("Telegram channel error: {}", e);
                }
            });
        }
    }

    // Start Discord channel if configured
    if let Some(ref discord_config) = config.channels.discord {
        let bot_token = &discord_config.bot_token;
        if !bot_token.is_empty() {
            info!("🎮 Starting Discord channel...");
            let discord_state = state.clone();
            let token = bot_token.clone();
            tokio::spawn(async move {
                if let Err(e) = run_discord_loop(token, discord_state).await {
                    tracing::error!("Discord channel error: {}", e);
                }
            });
        }
    }

    // Start Slack channel if configured
    if let Some(ref slack_config) = config.channels.slack {
        let bot_token = slack_config.bot_token.clone().unwrap_or_default();
        let app_token = slack_config.app_token.clone();
        if !bot_token.is_empty() {
            info!("💼 Starting Slack channel...");
            let slack_state = state.clone();
            tokio::spawn(async move {
                if let Err(e) = run_slack_loop(bot_token, app_token, slack_state).await {
                    tracing::error!("Slack channel error: {}", e);
                }
            });
        }
    }

    // Start cron scheduler with task processor
    {
        let state_clone = state.clone();
        tokio::spawn(async move {
            match crate::cron_scheduler::CronScheduler::load_jobs() {
                Ok(mut scheduler) => {
                    let job_count = scheduler.job_count();
                    if job_count > 0 {
                        info!("⏰ Cron scheduler started ({} jobs)", job_count);
                        let (cron_tx, mut cron_rx) = tokio::sync::mpsc::channel::<String>(16);
                        
                        // Spawn task processor
                        let processor_state = state_clone.clone();
                        tokio::spawn(async move {
                            while let Some(task) = cron_rx.recv().await {
                                info!("📋 Processing cron task: {}", task);
                                // Process the task through the agent
                                let state_guard = processor_state.read().await;
                                let config = &state_guard.config;
                                let workspace = config.workspace_dir().to_string_lossy().to_string();
                                
                                // Create a simple agent request for the task
                                let model = config.agents.defaults.model.clone()
                                    .unwrap_or_else(|| "anthropic/claude-sonnet-4-20250514".to_string());
                                
                                let tools_reg = klaw_tools::create_default_registry(None);
                                let tool_names: Vec<&str> = tools_reg.list();
                                let system_prompt = klaw_agent::SystemPromptBuilder::new(&workspace, &model)
                                    .with_tools(&tool_names)
                                    .with_channel("cron")
                                    .build();
                                
                                let agent_config = klaw_agent::AgentConfig {
                                    model,
                                    system_prompt,
                                    max_tool_rounds: 5,
                                    ..Default::default()
                                };
                                
                                let tool_ctx = klaw_tools::ToolContext {
                                    workspace_dir: workspace,
                                    session_key: "cron:main".to_string(),
                                    agent_id: "cron".to_string(),
                                };
                                
                                let key = SessionKey::main("cron");
                                let mut session = state_guard.session_store.get_or_create(&key, "cron").await;
                                drop(state_guard);
                                
                                // Run agent
                                match run_agent(
                                    state_clone.read().await.provider.as_ref(),
                                    &tools_reg,
                                    &mut session,
                                    &task,
                                    &agent_config,
                                    &tool_ctx,
                                ).await {
                                    Ok(result) => {
                                        let preview: String = result.response.chars().take(100).collect();
                                        info!("✅ Cron task completed: {}", preview);
                                    }
                                    Err(e) => {
                                        warn!("❌ Cron task failed: {}", e);
                                    }
                                }
                                
                                // Save session
                                state_clone.read().await.session_store.update(&session).await;
                            }
                        });
                        
                        scheduler.run(cron_tx).await;
                    } else {
                        info!("⏰ No cron jobs to schedule");
                    }
                }
                Err(e) => warn!("Cron scheduler load failed: {}", e),
            }
        });
    }

    // Start heartbeat if configured
    {
        let state_clone = state.clone();
        tokio::spawn(async move {
            let state_guard = state_clone.read().await;
            let config = &state_guard.config;
            
            // Check for heartbeat config in agent defaults
            if let Some(every) = &config.agents.defaults.heartbeat_every {
                let interval = match crate::heartbeat_parser::parse_heartbeat_interval(every) {
                    Some(d) => d,
                    None => {
                        warn!("Invalid heartbeat interval: {}", every);
                        return;
                    }
                };
                
                info!("💓 Heartbeat started: interval={}s", interval.as_secs());
                drop(state_guard);
                
                let mut interval_timer = tokio::time::interval(interval);
                interval_timer.tick().await; // Skip first tick
                
                loop {
                    interval_timer.tick().await;
                    info!("💓 Heartbeat tick");
                    
                    // Process heartbeat through agent
                    let state_guard = state_clone.read().await;
                    let config = &state_guard.config;
                    let workspace = config.workspace_dir().to_string_lossy().to_string();
                    let model = config.agents.defaults.model.clone()
                        .unwrap_or_else(|| "anthropic/claude-sonnet-4-20250514".to_string());
                    
                    let tools_reg = klaw_tools::create_default_registry(None);
                    let system_prompt = klaw_agent::SystemPromptBuilder::new(&workspace, &model)
                        .with_tools(&tools_reg.list())
                        .with_channel("heartbeat")
                        .build();
                    
                    let agent_config = klaw_agent::AgentConfig {
                        model,
                        system_prompt,
                        max_tool_rounds: 5,
                        ..Default::default()
                    };
                    
                    let tool_ctx = klaw_tools::ToolContext {
                        workspace_dir: workspace,
                        session_key: "heartbeat:main".to_string(),
                        agent_id: "heartbeat".to_string(),
                    };
                    
                    let key = SessionKey::main("heartbeat");
                    let mut session = state_guard.session_store.get_or_create(&key, "heartbeat").await;
                    
                    // Check for HEARTBEAT.md
                    let heartbeat_prompt = if std::path::Path::new(&config.workspace_dir()).join("HEARTBEAT.md").exists() {
                        if let Ok(content) = std::fs::read_to_string(config.workspace_dir().join("HEARTBEAT.md")) {
                            format!("Read and follow instructions from HEARTBEAT.md:\n\n{}", content)
                        } else {
                            "HEARTBEAT_OK".to_string()
                        }
                    } else {
                        "HEARTBEAT_OK".to_string()
                    };
                    
                    drop(state_guard);
                    
                    let _ = run_agent(
                        state_clone.read().await.provider.as_ref(),
                        &tools_reg,
                        &mut session,
                        &heartbeat_prompt,
                        &agent_config,
                        &tool_ctx,
                    ).await;
                    
                    state_clone.read().await.session_store.update(&session).await;
                }
            }
        });
    }

    let result = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await;

    // Cleanup lock file
    let _ = std::fs::remove_file(&lock_path);
    info!("Gateway shut down gracefully");
    result.map_err(|e| e.into())
}

fn create_llm_provider(config: &Config) -> anyhow::Result<Box<dyn LlmProvider>> {
    let model_str = config.agents.defaults.model.as_deref().unwrap_or("anthropic/claude-sonnet-4-20250514");
    let (provider, _model) = klaw_agent::create_provider(
        model_str,
        config.agents.defaults.api_key.as_deref(),
        config.agents.defaults.base_url.as_deref(),
        &std::collections::HashMap::new(),
    )?;
    Ok(provider)
}

async fn webchat_handler() -> Html<&'static str> {
    Html(include_str!("../../../webchat/index.html"))
}

async fn health_handler_with_state(state: Arc<RwLock<GatewayState>>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let uptime = s.started_at.elapsed().as_secs();
    let clients = s.presence.len();
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": uptime,
        "connected_clients": clients,
        "auth_enabled": s.gateway_token.is_some(),
        "paired_devices": s.pairing_store.devices.len(),
    }))
}

/// Canvas host — serves agent-editable HTML/CSS/JS
async fn canvas_handler() -> Html<&'static str> {
    Html(r#"<!DOCTYPE html><html><head><title>Klaw Canvas</title></head>
    <body style="margin:0;background:#1a1a2e;color:#e0e0f0;font-family:system-ui;display:flex;align-items:center;justify-content:center;height:100vh;">
    <div id="canvas-root"><h2>🎨 Klaw Canvas</h2><p>Canvas content will be rendered here.</p></div>
    <script>
    // Canvas will be controlled via gateway WS API (canvas.present, canvas.eval, etc.)
    const ws = new WebSocket(`ws://${location.host}/ws`);
    ws.onmessage = (e) => {
        const data = JSON.parse(e.data);
        if (data.type === 'event' && data.event === 'canvas.update') {
            document.getElementById('canvas-root').innerHTML = data.payload.html || '';
        }
    };
    </script></body></html>"#)
}

/// A2UI host — renders UI from JSONL instructions
async fn a2ui_handler() -> Html<&'static str> {
    Html(r#"<!DOCTYPE html><html><head><title>Klaw A2UI</title></head>
    <body style="margin:0;background:#1a1a2e;color:#e0e0f0;font-family:system-ui;display:flex;align-items:center;justify-content:center;height:100vh;">
    <div id="a2ui-root"><h2>🖼️ Klaw A2UI</h2><p>A2UI content will be rendered here.</p></div>
    </body></html>"#)
}

async fn webhook_handler(
    headers: HeaderMap,
    body: Bytes,
    state: Arc<RwLock<GatewayState>>,
) -> Json<serde_json::Value> {
    let s = state.read().await;
    let secret = s.config.gateway.token.clone();
    drop(s);

    let handler = crate::webhooks::WebhookHandler::new("/webhook", secret);

    // Validate signature if present
    let signature = headers.get("x-signature-256")
        .or_else(|| headers.get("x-hub-signature-256"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !handler.validate(&body, signature) {
        return Json(serde_json::json!({ "ok": false, "error": "Invalid signature" }));
    }

    // Parse body
    let body_value: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => return Json(serde_json::json!({ "ok": false, "error": format!("Invalid JSON: {}", e) })),
    };

    match handler.process(body_value) {
        Some(msg) => {
            info!("Webhook received message from {}: {:?}", msg.sender_id, msg.text);
            Json(serde_json::json!({ "ok": true, "message_id": msg.id }))
        }
        None => Json(serde_json::json!({ "ok": false, "error": "Could not extract message from payload" })),
    }
}

async fn handle_ws_connection(mut socket: WebSocket, state: Arc<RwLock<GatewayState>>) {
    info!("New WebSocket connection");
    let nonce = uuid::Uuid::new_v4().to_string();

    // Send challenge
    let challenge = serde_json::json!({
        "type": "event",
        "event": "connect.challenge",
        "payload": { "nonce": &nonce, "ts": chrono::Utc::now().timestamp_millis() }
    });
    let _ = socket.send(Message::Text(challenge.to_string().into())).await;

    let mut device_id = String::new();
    let mut client_role = "operator".to_string();
    let mut connected = false;
    let mut seq: u64 = 0;

    // Tick timer
    let mut tick_interval = tokio::time::interval(std::time::Duration::from_secs(15));

    loop {
        tokio::select! {
            // Tick events
            _ = tick_interval.tick() => {
                if connected {
                    seq += 1;
                    let tick = serde_json::json!({
                        "type": "event", "event": "tick", "seq": seq,
                        "payload": { "ts": chrono::Utc::now().timestamp_millis() }
                    });
                    if socket.send(Message::Text(tick.to_string().into())).await.is_err() {
                        break;
                    }
                }
            }
            // Client messages
            msg = socket.recv() => {
                let msg = match msg {
                    Some(Ok(m)) => m,
                    _ => break,
                };

                let text = match msg {
                    Message::Text(t) => t.to_string(),
                    Message::Close(_) => { info!("Client disconnected"); break; }
                    _ => continue,
                };

                let frame: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(f) => f,
                    Err(e) => {
                        // Non-JSON first frame = hard close (invariant)
                        if !connected { warn!("Non-JSON first frame, closing: {}", e); break; }
                        warn!("Invalid frame: {}", e);
                        continue;
                    }
                };

                let frame_type = frame.get("type").and_then(|t| t.as_str()).unwrap_or("");
                let method = frame.get("method").and_then(|m| m.as_str()).unwrap_or("");
                let id = frame.get("id").and_then(|i| i.as_str()).unwrap_or("0").to_string();

                // First frame MUST be connect
                if !connected && method != "connect" {
                    warn!("First frame not connect, closing");
                    let _ = socket.send(Message::Text(serde_json::json!({
                        "type": "res", "id": id, "ok": false,
                        "error": { "code": "connect_required", "message": "First frame must be connect" }
                    }).to_string().into())).await;
                    break;
                }

                match (frame_type, method) {
                    ("req", "connect") => {
                        let params = &frame["params"];

                        // Auth token check
                        {
                            let s = state.read().await;
                            if let Some(ref expected_token) = s.gateway_token {
                                let provided = params["auth"]["token"].as_str().unwrap_or("");
                                if provided != expected_token {
                                    warn!("Auth failed — invalid token");
                                    let _ = socket.send(Message::Text(serde_json::json!({
                                        "type": "res", "id": id, "ok": false,
                                        "error": { "code": "auth_failed", "message": "Invalid gateway token" }
                                    }).to_string().into())).await;
                                    break;
                                }
                            }
                        }

                        // Extract device identity
                        client_role = params["role"].as_str().unwrap_or("operator").to_string();
                        device_id = params["device"]["id"].as_str()
                            .or(params["client"]["id"].as_str())
                            .unwrap_or("unknown")
                            .to_string();

                        let platform = params["client"]["platform"].as_str().unwrap_or("unknown");
                        let version = params["client"]["version"].as_str().unwrap_or("0.0.0");

                        // Check pairing for non-local devices
                        let is_local = device_id == "cli" || device_id == "webchat";
                        let mut device_token_to_issue: Option<String> = None;

                        if client_role == "node" && !is_local {
                            let mut s = state.write().await;
                            if !s.pairing_store.is_paired(&device_id) {
                                // Auto-approve local, queue others
                                info!("New device needs pairing: {} ({})", device_id, platform);
                                s.pairing_store.pending.insert(device_id.clone(), DeviceIdentity {
                                    device_id: device_id.clone(),
                                    platform: platform.to_string(),
                                    role: client_role.clone(),
                                    client_id: params["client"]["id"].as_str().unwrap_or("").to_string(),
                                    client_version: version.to_string(),
                                    caps: params["caps"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()).unwrap_or_default(),
                                    commands: params["commands"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()).unwrap_or_default(),
                                    paired: false,
                                    device_token: None,
                                });
                                s.pairing_store.save();
                            } else {
                                device_token_to_issue = s.pairing_store.devices.get(&device_id)
                                    .and_then(|d| d.device_token.clone());
                            }
                        } else if is_local {
                            // Auto-approve local connections
                            let token = uuid::Uuid::new_v4().to_string();
                            device_token_to_issue = Some(token);
                        }

                        // Track presence
                        {
                            let mut s = state.write().await;
                            s.presence.insert(device_id.clone(), PresenceInfo {
                                device_id: device_id.clone(),
                                role: client_role.clone(),
                                connected_at: chrono::Utc::now().timestamp_millis(),
                                last_seen: chrono::Utc::now().timestamp_millis(),
                            });
                        }

                        // Build hello-ok response
                        let mut payload = serde_json::json!({
                            "type": "hello-ok",
                            "protocol": 3,
                            "policy": { "tickIntervalMs": 15000 },
                            "health": { "status": "ok", "version": env!("CARGO_PKG_VERSION") },
                        });

                        if let Some(token) = device_token_to_issue {
                            payload["auth"] = serde_json::json!({
                                "deviceToken": token,
                                "role": client_role,
                                "scopes": ["operator.read", "operator.write"],
                            });
                        }

                        // Add presence snapshot
                        {
                            let s = state.read().await;
                            payload["presence"] = serde_json::json!(
                                s.presence.values().collect::<Vec<_>>()
                            );
                        }

                        let _ = socket.send(Message::Text(serde_json::json!({
                            "type": "res", "id": id, "ok": true, "payload": payload
                        }).to_string().into())).await;

                        connected = true;
                        info!("Client connected: {} (role: {}, platform: {})", device_id, client_role, platform);

                        // Send presence event to all (would need broadcast channel in production)
                        seq += 1;
                        let presence_event = serde_json::json!({
                            "type": "event", "event": "presence", "seq": seq,
                            "payload": { "device_id": &device_id, "role": &client_role, "status": "online" }
                        });
                        let _ = socket.send(Message::Text(presence_event.to_string().into())).await;
                    }

                    ("req", "agent") => {
                        let user_msg = frame["params"]["message"].as_str().unwrap_or("");
                        if user_msg.is_empty() {
                            let _ = socket.send(Message::Text(serde_json::json!({
                                "type": "res", "id": id, "ok": false,
                                "error": { "code": "missing_message", "message": "No message provided" }
                            }).to_string().into())).await;
                            continue;
                        }

                        // Slash command handling
                        let session_key = SessionKey::main("default");
                        match handle_slash_command(user_msg, &state, &session_key).await {
                            SlashCommand::Handled(response) => {
                                let _ = socket.send(Message::Text(serde_json::json!({
                                    "type": "res", "id": id, "ok": true,
                                    "payload": {
                                        "runId": uuid::Uuid::new_v4().to_string(),
                                        "status": "ok",
                                        "response": response,
                                        "isCommand": true
                                    }
                                }).to_string().into())).await;
                                continue;
                            }
                            SlashCommand::NotASlashCommand | SlashCommand::ContinueToAgent => {
                                // Continue to regular agent processing
                            }
                        }

                        // Idempotency check
                        let idemp_key = frame["params"]["idempotencyKey"].as_str().map(|s| s.to_string());
                        if let Some(ref key) = idemp_key {
                            let mut s = state.write().await;
                            if let Some(cached) = s.idempotency.get(key) {
                                let _ = socket.send(Message::Text(cached.to_string().into())).await;
                                continue;
                            }
                        }

                        let run_id = uuid::Uuid::new_v4().to_string();
                        info!("Agent request [{}]: {}", run_id, user_msg);

                        // Send accepted ack
                        let _ = socket.send(Message::Text(serde_json::json!({
                            "type": "res", "id": &id, "ok": true,
                            "payload": { "runId": &run_id, "status": "accepted", "acceptedAt": chrono::Utc::now().timestamp_millis() }
                        }).to_string().into())).await;

                        // Send lifecycle start event
                        seq += 1;
                        let _ = socket.send(Message::Text(serde_json::json!({
                            "type": "event", "event": "agent", "seq": seq,
                            "payload": { "runId": &run_id, "stream": "lifecycle", "phase": "start" }
                        }).to_string().into())).await;

                        // Run agent
                        let state_guard = state.read().await;
                        let config = &state_guard.config;
                        let model = config.agents.defaults.model.clone()
                            .unwrap_or_else(|| "anthropic/claude-sonnet-4-20250514".to_string());

                        // Extract model name without provider prefix for the agent
                        let model_for_agent = if model.contains('/') {
                            model.split('/').last().unwrap_or(&model).to_string()
                        } else {
                            model.clone()
                        };

                        let workspace = config.workspace_dir().to_string_lossy().to_string();
                        let tools_reg = create_default_registry(None);
                        let tool_names: Vec<&str> = tools_reg.list();
                        let system_prompt = SystemPromptBuilder::new(&workspace, &model_for_agent)
                            .with_tools(&tool_names)
                            .with_channel("webchat")
                            .build();

                        let agent_config = AgentConfig {
                            model: model_for_agent,
                            system_prompt,
                            max_tool_rounds: 10,
                            temperature: None,
                            max_tokens: Some(4096),
                            failover_models: config.agents.defaults.failover.clone(),
                            api_keys: config.agents.defaults.api_keys.clone(),
                            retry_count: config.agents.defaults.retry_count.unwrap_or(2),
                            retry_delay: std::time::Duration::from_millis(
                                config.agents.defaults.retry_delay_ms.unwrap_or(1000),
                            ),
                            ..AgentConfig::default()
                        };

                        let tool_ctx = ToolContext {
                            workspace_dir: workspace,
                            session_key: "agent:default:main".to_string(),
                            agent_id: "default".to_string(),
                        };

                        let key = SessionKey::main("default");
                        let mut session = state_guard.session_store.get_or_create(&key, "default").await;

                        // Check if streaming is requested
                        let stream_mode = frame["params"]["stream"].as_bool().unwrap_or(false);

                        let result = if stream_mode {
                            // Streaming mode - forward chunks to WebSocket
                            let (tx, mut rx) = tokio::sync::mpsc::channel::<klaw_core::types::StreamChunk>(64);
                            let (ws_tx, mut ws_rx) = tokio::sync::mpsc::channel::<String>(64);
                            let run_id_clone = run_id.clone();

                            // Spawn task to forward stream chunks to ws_tx channel
                            let run_id_for_forward = run_id_clone.clone();
                            let forwarder = tokio::spawn(async move {
                                while let Some(chunk) = rx.recv().await {
                                    let event = match &chunk {
                                        klaw_core::types::StreamChunk::Text(text) => serde_json::json!({
                                            "type": "event", "event": "agent", "runId": run_id_for_forward,
                                            "payload": { "stream": "text", "delta": text }
                                        }),
                                        klaw_core::types::StreamChunk::ToolCallStart { id, name } => serde_json::json!({
                                            "type": "event", "event": "agent", "runId": run_id_for_forward,
                                            "payload": { "stream": "tool_start", "toolId": id, "name": name }
                                        }),
                                        klaw_core::types::StreamChunk::ToolCallDelta { id, arguments } => serde_json::json!({
                                            "type": "event", "event": "agent", "runId": run_id_for_forward,
                                            "payload": { "stream": "tool_delta", "toolId": id, "delta": arguments }
                                        }),
                                        klaw_core::types::StreamChunk::ToolCallEnd { id } => serde_json::json!({
                                            "type": "event", "event": "agent", "runId": run_id_for_forward,
                                            "payload": { "stream": "tool_end", "toolId": id }
                                        }),
                                        klaw_core::types::StreamChunk::Done { usage } => serde_json::json!({
                                            "type": "event", "event": "agent", "runId": run_id_for_forward,
                                            "payload": { "stream": "done", "usage": usage }
                                        }),
                                        klaw_core::types::StreamChunk::Error(e) => serde_json::json!({
                                            "type": "event", "event": "agent", "runId": run_id_for_forward,
                                            "payload": { "stream": "error", "error": e }
                                        }),
                                    };
                                    if ws_tx.send(event.to_string()).await.is_err() {
                                        break;
                                    }
                                }
                            });

                            // Run agent with streaming
                            let result = klaw_agent::run_agent_streaming(
                                state_guard.provider.as_ref(),
                                &tools_reg,
                                &mut session,
                                user_msg,
                                &agent_config,
                                &tool_ctx,
                                tx,
                            ).await;

                            // Forward any remaining ws messages
                            while let Some(msg) = ws_rx.recv().await {
                                let _ = socket.send(Message::Text(msg.into())).await;
                            }

                            // Wait for forwarder to finish
                            let _ = forwarder.await;

                            result
                        } else {
                            // Non-streaming mode
                            run_agent(
                                state_guard.provider.as_ref(),
                                &tools_reg,
                                &mut session,
                                user_msg,
                                &agent_config,
                                &tool_ctx,
                            ).await
                        };

                        state_guard.session_store.update(&session).await;
                        drop(state_guard);

                        match result {
                            Ok(agent_result) => {
                                let response = serde_json::json!({
                                    "type": "res", "id": &id, "ok": true,
                                    "payload": {
                                        "runId": &run_id,
                                        "status": "ok",
                                        "response": agent_result.response,
                                        "tool_calls": agent_result.tool_calls_made,
                                        "usage": {
                                            "input_tokens": agent_result.input_tokens,
                                            "output_tokens": agent_result.output_tokens,
                                        }
                                    }
                                });

                                // Cache for idempotency
                                if let Some(ref key) = idemp_key {
                                    let mut s = state.write().await;
                                    s.idempotency.set(key.clone(), response.clone());
                                }

                                let _ = socket.send(Message::Text(response.to_string().into())).await;

                                // Lifecycle end event
                                seq += 1;
                                let _ = socket.send(Message::Text(serde_json::json!({
                                    "type": "event", "event": "agent", "seq": seq,
                                    "payload": { "runId": &run_id, "stream": "lifecycle", "phase": "end" }
                                }).to_string().into())).await;
                            }
                            Err(e) => {
                                warn!("Agent error: {}", e);
                                let _ = socket.send(Message::Text(serde_json::json!({
                                    "type": "res", "id": &id, "ok": false,
                                    "error": { "code": "agent_error", "message": e.to_string() }
                                }).to_string().into())).await;

                                // Lifecycle error event
                                seq += 1;
                                let _ = socket.send(Message::Text(serde_json::json!({
                                    "type": "event", "event": "agent", "seq": seq,
                                    "payload": { "runId": &run_id, "stream": "lifecycle", "phase": "error", "error": e.to_string() }
                                }).to_string().into())).await;
                            }
                        }
                    }

                    ("req", "health") => {
                        let s = state.read().await;
                        let _ = socket.send(Message::Text(serde_json::json!({
                            "type": "res", "id": id, "ok": true,
                            "payload": {
                                "status": "ok",
                                "version": env!("CARGO_PKG_VERSION"),
                                "uptime_seconds": s.started_at.elapsed().as_secs(),
                                "connected_clients": s.presence.len(),
                            }
                        }).to_string().into())).await;
                    }

                    ("req", "status") => {
                        let s = state.read().await;
                        let _ = socket.send(Message::Text(serde_json::json!({
                            "type": "res", "id": id, "ok": true,
                            "payload": {
                                "version": env!("CARGO_PKG_VERSION"),
                                "uptime_seconds": s.started_at.elapsed().as_secs(),
                                "gateway": "running",
                                "provider": s.provider.name(),
                                "model": s.config.agents.defaults.model,
                                "connected_clients": s.presence.len(),
                                "paired_devices": s.pairing_store.devices.len(),
                                "pending_devices": s.pairing_store.pending.len(),
                            }
                        }).to_string().into())).await;
                    }

                    ("req", "system-presence") => {
                        // Update presence
                        let mut s = state.write().await;
                        if let Some(p) = s.presence.get_mut(&device_id) {
                            p.last_seen = chrono::Utc::now().timestamp_millis();
                        }
                        let _ = socket.send(Message::Text(serde_json::json!({
                            "type": "res", "id": id, "ok": true, "payload": {}
                        }).to_string().into())).await;
                    }

                    ("req", "pairing.list") => {
                        let s = state.read().await;
                        let _ = socket.send(Message::Text(serde_json::json!({
                            "type": "res", "id": id, "ok": true,
                            "payload": {
                                "paired": s.pairing_store.devices.keys().collect::<Vec<_>>(),
                                "pending": s.pairing_store.pending.keys().collect::<Vec<_>>(),
                            }
                        }).to_string().into())).await;
                    }

                    ("req", "pairing.approve") => {
                        let target_device = frame["params"]["deviceId"].as_str().unwrap_or("");
                        let mut s = state.write().await;
                        if let Some(token) = s.pairing_store.approve(target_device) {
                            info!("Approved device: {}", target_device);
                            let _ = socket.send(Message::Text(serde_json::json!({
                                "type": "res", "id": id, "ok": true,
                                "payload": { "deviceToken": token }
                            }).to_string().into())).await;
                        } else {
                            let _ = socket.send(Message::Text(serde_json::json!({
                                "type": "res", "id": id, "ok": false,
                                "error": { "code": "not_found", "message": "No pending device" }
                            }).to_string().into())).await;
                        }
                    }

                    ("req", "pairing.reject") => {
                        let target_device = frame["params"]["deviceId"].as_str().unwrap_or("");
                        let mut s = state.write().await;
                        s.pairing_store.reject(target_device);
                        let _ = socket.send(Message::Text(serde_json::json!({
                            "type": "res", "id": id, "ok": true, "payload": {}
                        }).to_string().into())).await;
                    }

                    _ => {
                        let _ = socket.send(Message::Text(serde_json::json!({
                            "type": "res", "id": id, "ok": false,
                            "error": { "code": "unknown_method", "message": format!("Unknown: {}", method) }
                        }).to_string().into())).await;
                    }
                }
            }
        }
    }

    // Cleanup presence on disconnect
    {
        let mut s = state.write().await;
        s.presence.remove(&device_id);
    }
    info!("Connection closed: {}", device_id);
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.expect("Failed to install Ctrl+C handler");
    info!("Received shutdown signal");
}

/// Telegram long-polling loop — receives messages and runs agent
async fn run_telegram_loop(bot_token: String, state: Arc<RwLock<GatewayState>>) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let api_base = format!("https://api.telegram.org/bot{}", bot_token);

    // Get bot info
    let me: serde_json::Value = client.get(&format!("{}/getMe", api_base))
        .send().await?.json().await?;
    let bot_username = me["result"]["username"].as_str().unwrap_or("bot");
    info!("🤖 Telegram bot connected: @{}", bot_username);

    // Set bot commands
    let _ = client.post(&format!("{}/setMyCommands", api_base))
        .json(&serde_json::json!({
            "commands": [
                {"command": "start", "description": "Start chatting"},
                {"command": "new", "description": "New conversation"},
                {"command": "help", "description": "Show help"},
                {"command": "status", "description": "Show status"},
            ]
        }))
        .send().await;

    let mut offset: i64 = 0;

    loop {
        // Long poll for updates
        let resp = client.post(&format!("{}/getUpdates", api_base))
            .json(&serde_json::json!({
                "offset": offset,
                "timeout": 30,
                "allowed_updates": ["message", "edited_message", "callback_query"]
            }))
            .send()
            .await;

        let data: serde_json::Value = match resp {
            Ok(r) => match r.json().await {
                Ok(d) => d,
                Err(e) => { warn!("Telegram JSON error: {}", e); tokio::time::sleep(std::time::Duration::from_secs(5)).await; continue; }
            },
            Err(e) => { warn!("Telegram poll error: {}", e); tokio::time::sleep(std::time::Duration::from_secs(5)).await; continue; }
        };

        let updates = match data["result"].as_array() {
            Some(u) => u.clone(),
            None => continue,
        };

        for update in &updates {
            offset = update["update_id"].as_i64().unwrap_or(0) + 1;

            let msg = match update.get("message") {
                Some(m) => m,
                None => continue,
            };

            let chat_id = msg["chat"]["id"].as_i64().unwrap_or(0);
            let sender_id = msg["from"]["id"].as_i64().unwrap_or(0);
            let sender_name = msg["from"]["first_name"].as_str().unwrap_or("Unknown");
            let text = match msg["text"].as_str() {
                Some(t) => t.to_string(),
                None => continue, // Skip non-text for now
            };

            info!("📩 Telegram [{} / {}]: {}", sender_name, chat_id, text);

            // Handle /start command
            if text == "/start" {
                let _ = client.post(&format!("{}/sendMessage", api_base))
                    .json(&serde_json::json!({
                        "chat_id": chat_id,
                        "text": "👋 Hello! I'm Klaw, your AI assistant. Send me a message to get started!",
                        "parse_mode": "Markdown"
                    }))
                    .send().await;
                continue;
            }

            // Handle /new and /reset
            if text == "/new" || text == "/reset" {
                // TODO: Reset session
                let _ = client.post(&format!("{}/sendMessage", api_base))
                    .json(&serde_json::json!({
                        "chat_id": chat_id,
                        "text": "🔄 Conversation reset!",
                    }))
                    .send().await;
                continue;
            }

            // Handle /status
            if text == "/status" {
                let s = state.read().await;
                let uptime = s.started_at.elapsed().as_secs();
                let model = s.config.agents.defaults.model.as_deref().unwrap_or("unknown");
                let _ = client.post(&format!("{}/sendMessage", api_base))
                    .json(&serde_json::json!({
                        "chat_id": chat_id,
                        "text": format!("🦀 *Klaw Gateway*\n• Version: {}\n• Uptime: {}s\n• Model: `{}`\n• Provider: {}", 
                            env!("CARGO_PKG_VERSION"), uptime, model, s.provider.name()),
                        "parse_mode": "Markdown"
                    }))
                    .send().await;
                continue;
            }

            // Handle /help
            if text == "/help" {
                let _ = client.post(&format!("{}/sendMessage", api_base))
                    .json(&serde_json::json!({
                        "chat_id": chat_id,
                        "text": "🦀 *Klaw Commands:*\n/start - Start chatting\n/new - Reset conversation\n/status - Show status\n/help - This message\n\nJust send any message to chat with me!",
                        "parse_mode": "Markdown"
                    }))
                    .send().await;
                continue;
            }

            // Send typing indicator
            let _ = client.post(&format!("{}/sendChatAction", api_base))
                .json(&serde_json::json!({
                    "chat_id": chat_id,
                    "action": "typing"
                }))
                .send().await;

            // Run agent
            let response_text = {
                let state_guard = state.read().await;
                let config = &state_guard.config;
                let model = config.agents.defaults.model.clone()
                    .unwrap_or_else(|| "anthropic/claude-sonnet-4-20250514".to_string());
                let model_for_agent = if model.contains('/') {
                    model.split('/').last().unwrap_or(&model).to_string()
                } else {
                    model.clone()
                };

                let workspace = config.workspace_dir().to_string_lossy().to_string();
                let tools_reg = create_default_registry(None);
                let tool_names: Vec<&str> = tools_reg.list();
                let system_prompt = SystemPromptBuilder::new(&workspace, &model_for_agent)
                    .with_tools(&tool_names)
                    .with_channel("telegram")
                    .build();

                let agent_config = AgentConfig {
                    model: model_for_agent,
                    system_prompt,
                    max_tool_rounds: 10,
                    temperature: None,
                    max_tokens: Some(4096),
                    failover_models: config.agents.defaults.failover.clone(),
                    api_keys: config.agents.defaults.api_keys.clone(),
                    retry_count: config.agents.defaults.retry_count.unwrap_or(2),
                    retry_delay: std::time::Duration::from_millis(
                        config.agents.defaults.retry_delay_ms.unwrap_or(1000),
                    ),
                    ..AgentConfig::default()
                };

                let tool_ctx = ToolContext {
                    workspace_dir: workspace,
                    session_key: format!("agent:default:telegram:{}", sender_id),
                    agent_id: "default".to_string(),
                };

                let key = SessionKey::group("default", "telegram", &sender_id.to_string());
                let mut session = state_guard.session_store.get_or_create(&key, "default").await;

                let result = run_agent(
                    state_guard.provider.as_ref(),
                    &tools_reg,
                    &mut session,
                    &text,
                    &agent_config,
                    &tool_ctx,
                ).await;

                state_guard.session_store.update(&session).await;

                match result {
                    Ok(r) => r.response,
                    Err(e) => format!("❌ Error: {}", e),
                }
            };

            // Send response (chunked for Telegram's 4096 limit)
            let chunks = chunk_text_for_telegram(&response_text);
            for chunk in &chunks {
                let _ = client.post(&format!("{}/sendMessage", api_base))
                    .json(&serde_json::json!({
                        "chat_id": chat_id,
                        "text": chunk,
                        "parse_mode": "Markdown",
                        "reply_to_message_id": msg["message_id"].as_i64().unwrap_or(0)
                    }))
                    .send().await;
            }

            info!("📤 Telegram response sent to {}", sender_name);
        }
    }
}

fn chunk_text_for_telegram(text: &str) -> Vec<String> {
    const MAX_LEN: usize = 4096;
    if text.len() <= MAX_LEN {
        return vec![text.to_string()];
    }
    let mut chunks = Vec::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        if remaining.len() <= MAX_LEN {
            chunks.push(remaining.to_string());
            break;
        }
        let split_at = remaining[..MAX_LEN].rfind('\n').unwrap_or(MAX_LEN);
        chunks.push(remaining[..split_at].to_string());
        remaining = remaining[split_at..].trim_start();
    }
    chunks
}

/// Slash command handler — returns Some(response) if command was handled
enum SlashCommand {
    Handled(String),
    NotASlashCommand,
    ContinueToAgent,  // Slash command but should still go to agent
}

async fn handle_slash_command(
    text: &str,
    state: &Arc<RwLock<GatewayState>>,
    session_key: &SessionKey,
) -> SlashCommand {
    let text = text.trim();
    if !text.starts_with('/') {
        return SlashCommand::NotASlashCommand;
    }

    let parts: Vec<&str> = text.splitn(2, ' ').collect();
    let command = parts[0].to_lowercase();
    let args = parts.get(1).unwrap_or(&"");

    match command.as_str() {
        "/help" | "/commands" => {
            SlashCommand::Handled(
                "🦀 *Klaw Commands:*\n\
                /help — Show this help\n\
                /status — Show gateway status\n\
                /model — Show current model\n\
                /model <name> — Switch model\n\
                /new — Start new conversation\n\
                /reset — Reset current session\n\
                /clear — Clear screen (WebChat)\n\
                /usage — Show token usage\n\
                /agents — List agents\n\
                /version — Show version\n".to_string()
            )
        }

        "/status" => {
            let s = state.read().await;
            let uptime = s.started_at.elapsed().as_secs();
            SlashCommand::Handled(format!(
                "🦀 *Klaw Gateway Status*\n\
                • Version: {}\n\
                • Uptime: {}s\n\
                • Provider: {}\n\
                • Model: `{}`\n\
                • Clients: {}\n\
                • Paired devices: {}",
                env!("CARGO_PKG_VERSION"),
                uptime,
                s.provider.name(),
                s.config.agents.defaults.model.as_deref().unwrap_or("unknown"),
                s.presence.len(),
                s.pairing_store.devices.len()
            ))
        }

        "/model" => {
            if args.is_empty() {
                let s = state.read().await;
                let current = s.config.agents.defaults.model.as_deref().unwrap_or("unknown");
                let provider = s.provider.name();
                SlashCommand::Handled(format!(
                    "📊 *Current Model:*\n\
                    • Model: `{}`\n\
                    • Provider: {}\n\n\
                    To change: `/model <provider/model>`\n\
                    Example: `/model anthropic/claude-sonnet-4-20250514`",
                    current, provider
                ))
            } else {
                // Update model in config
                let mut s = state.write().await;
                let new_model = args.trim().to_string();
                s.config.agents.defaults.model = Some(new_model.clone());
                SlashCommand::Handled(format!("✅ Model switched to: `{}`", new_model))
            }
        }

        "/new" | "/reset" => {
            let s = state.write().await;
            let deleted = s.session_store.delete(&session_key.0).await;
            if deleted {
                SlashCommand::Handled("🔄 Session reset! Starting fresh conversation.".to_string())
            } else {
                SlashCommand::Handled("✅ New session started!".to_string())
            }
        }

        "/usage" => {
            let s = state.read().await;
            if let Some(session) = s.session_store.get(&session_key.0).await {
                SlashCommand::Handled(format!(
                    "📊 *Session Usage:*\n\
                    • Input tokens: {}\n\
                    • Output tokens: {}\n\
                    • Total tokens: {}\n\
                    • Messages: {}",
                    session.meta.input_tokens,
                    session.meta.output_tokens,
                    session.meta.total_tokens,
                    session.meta.message_count
                ))
            } else {
                SlashCommand::Handled("📊 No session usage data yet.".to_string())
            }
        }

        "/version" => {
            SlashCommand::Handled(format!(
                "🦀 Klaw v{}\n\
                • Rust Multi-Agent Platform\n\
                • Gateway + CLI + Channels",
                env!("CARGO_PKG_VERSION")
            ))
        }

        "/agents" => {
            SlashCommand::Handled(
                "📋 *Agents:*\n\
                • `default` — Main agent (active)\n\n\
                Multi-agent support coming soon!".to_string()
            )
        }

        // Unknown slash command — could be a custom command or just let agent handle
        _ => SlashCommand::ContinueToAgent
    }
}

/// Run Discord Gateway WebSocket loop
async fn run_discord_loop(bot_token: String, state: Arc<RwLock<GatewayState>>) -> anyhow::Result<()> {
    use klaw_channels::Channel;
    use klaw_channels::channels::discord::DiscordChannel;
    
    let (tx, mut rx) = tokio::sync::mpsc::channel::<klaw_core::types::InboundMessage>(64);
    
    let mut channel = DiscordChannel::new(&bot_token);
    
    // Spawn the channel start in a separate task
    let channel_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = channel.start(tx).await {
            tracing::error!("Discord channel error: {}", e);
        }
    });
    
    // Process incoming messages
    while let Some(msg) = rx.recv().await {
        info!("📩 Discord [{} / {}]: {:?}", msg.sender_name.as_deref().unwrap_or("?"), msg.chat_id, msg.text);
        
        // Handle slash commands
        if let Some(ref text) = msg.text {
            if text.starts_with('/') {
                let session_key = SessionKey::group("discord", "discord", &msg.chat_id);
                match handle_slash_command(text, &channel_state, &session_key).await {
                    SlashCommand::Handled(response) => {
                        // Send response back to Discord
                        let client = reqwest::Client::new();
                        let _ = client.post(&format!("https://discord.com/api/v10/channels/{}/messages", msg.chat_id))
                            .header("Authorization", format!("Bot {}", bot_token))
                            .json(&serde_json::json!({ "content": response }))
                            .send().await;
                        continue;
                    }
                    _ => {}
                }
            }
        }
        
        // Run agent with the message
        // TODO: Implement agent call similar to Telegram
    }
    
    Ok(())
}

/// Run Slack Socket Mode loop
async fn run_slack_loop(bot_token: String, app_token: Option<String>, state: Arc<RwLock<GatewayState>>) -> anyhow::Result<()> {
    use klaw_channels::Channel;
    use klaw_channels::channels::slack::SlackChannel;
    
    let (tx, mut rx) = tokio::sync::mpsc::channel::<klaw_core::types::InboundMessage>(64);
    
    let mut channel = SlackChannel::new(&bot_token, app_token);
    
    // Spawn the channel start in a separate task
    let slack_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = channel.start(tx).await {
            tracing::error!("Slack channel error: {}", e);
        }
    });
    
    // Process incoming messages
    while let Some(msg) = rx.recv().await {
        info!("📩 Slack [{} / {}]: {:?}", msg.sender_name.as_deref().unwrap_or("?"), msg.chat_id, msg.text);
        
        // Handle slash commands
        if let Some(ref text) = msg.text {
            if text.starts_with('/') {
                let session_key = SessionKey::group("slack", "slack", &msg.chat_id);
                match handle_slash_command(text, &slack_state, &session_key).await {
                    SlashCommand::Handled(response) => {
                        // Send response back to Slack
                        let client = reqwest::Client::new();
                        let _ = client.post("https://slack.com/api/chat.postMessage")
                            .header("Authorization", format!("Bearer {}", bot_token))
                            .json(&serde_json::json!({ 
                                "channel": msg.chat_id, 
                                "text": response 
                            }))
                            .send().await;
                        continue;
                    }
                    _ => {}
                }
            }
        }
        
        // Run agent with the message
        // TODO: Implement agent call similar to Telegram
    }
    
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// OpenAI-Compatible API Handlers
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, serde::Deserialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    max_tokens: Option<u64>,
    #[serde(default)]
    stream: Option<bool>,
    #[serde(default)]
    tools: Option<Vec<ToolDef>>,
}

#[derive(Debug, serde::Deserialize)]
struct ChatMessage {
    role: String,
    content: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, serde::Deserialize)]
struct ToolDef {
    #[serde(rename = "type")]
    tool_type: String,
    function: FunctionDef,
}

#[derive(Debug, serde::Deserialize)]
struct FunctionDef {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    parameters: Option<serde_json::Value>,
}

/// OpenAI-compatible chat completions endpoint
async fn chat_completions_handler(
    headers: HeaderMap,
    body: Bytes,
    state: Arc<RwLock<GatewayState>>,
) -> Json<serde_json::Value> {
    // Check auth
    if let Some(token) = &state.read().await.gateway_token {
        let auth = headers.get("Authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !auth.starts_with("Bearer ") || &auth[7..] != token {
            return Json(serde_json::json!({
                "error": { "message": "Unauthorized", "type": "invalid_request_error" }
            }));
        }
    }

    // Parse request
    let req: ChatCompletionRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return Json(serde_json::json!({
            "error": { "message": format!("Invalid request: {}", e), "type": "invalid_request_error" }
        })),
    };

    info!("OpenAI API request: model={}, messages={}", req.model, req.messages.len());

    // Convert messages
    let messages: Vec<klaw_core::types::Message> = req.messages.iter().map(|m| {
        match m.role.as_str() {
            "system" => klaw_core::types::Message::system(m.content.as_deref().unwrap_or("")),
            "user" => klaw_core::types::Message::user(m.content.as_deref().unwrap_or("")),
            "assistant" => klaw_core::types::Message::assistant(m.content.as_deref().unwrap_or("")),
            _ => klaw_core::types::Message::user(m.content.as_deref().unwrap_or("")),
        }
    }).collect();

    // Create chat request
    let chat_req = klaw_agent::provider::ChatRequest {
        model: req.model.clone(),
        messages,
        tools: None,
        temperature: req.temperature,
        max_tokens: req.max_tokens.map(|v| v as u32),
        stream: req.stream.unwrap_or(false),
        thinking: None,
    };

    // Call provider
    let s = state.read().await;
    match s.provider.chat(chat_req).await {
        Ok(response) => {
            Json(serde_json::json!({
                "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
                "object": "chat.completion",
                "created": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                "model": req.model,
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": response.content
                    },
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": response.usage.input_tokens,
                    "completion_tokens": response.usage.output_tokens,
                    "total_tokens": response.usage.total_tokens
                }
            }))
        }
        Err(e) => Json(serde_json::json!({
            "error": { "message": format!("Provider error: {}", e), "type": "api_error" }
        })),
    }
}

/// List available models (OpenAI-compatible)
async fn models_handler(state: Arc<RwLock<GatewayState>>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let default_model = s.config.agents.defaults.model.clone().unwrap_or_default();
    
    Json(serde_json::json!({
        "object": "list",
        "data": [
            {
                "id": default_model,
                "object": "model",
                "created": 1700000000,
                "owned_by": "klaw"
            },
            {
                "id": "claude-sonnet-4-20250514",
                "object": "model",
                "created": 1700000000,
                "owned_by": "anthropic"
            },
            {
                "id": "gpt-4",
                "object": "model",
                "created": 1700000000,
                "owned_by": "openai"
            }
        ]
    }))
}

/// Invoke tools directly
#[derive(Debug, serde::Deserialize)]
struct ToolsInvokeRequest {
    tool: String,
    params: serde_json::Value,
    #[serde(default)]
    workspace: Option<String>,
}

async fn tools_invoke_handler(
    headers: HeaderMap,
    body: Bytes,
    state: Arc<RwLock<GatewayState>>,
) -> Json<serde_json::Value> {
    // Check auth
    if let Some(token) = &state.read().await.gateway_token {
        let auth = headers.get("Authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !auth.starts_with("Bearer ") || &auth[7..] != token {
            return Json(serde_json::json!({
                "error": { "message": "Unauthorized" }
            }));
        }
    }

    // Parse request
    let req: ToolsInvokeRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return Json(serde_json::json!({
            "error": { "message": format!("Invalid request: {}", e) }
        })),
    };

    info!("Tools invoke: tool={}", req.tool);

    // Create tool registry
    let s = state.read().await;
    let workspace = req.workspace.clone().unwrap_or_else(|| s.config.workspace_dir().to_string_lossy().to_string());
    drop(s);
    let registry = klaw_tools::create_default_registry(None);
    
    // Find tool
    let tool = match registry.get(&req.tool) {
        Some(t) => t,
        None => return Json(serde_json::json!({
            "error": { "message": format!("Tool not found: {}", req.tool) }
        })),
    };

    // Create context
    let ctx = klaw_tools::ToolContext {
        workspace_dir: workspace,
        session_key: "api".to_string(),
        agent_id: "default".to_string(),
    };

    // Execute tool
    match tool.execute(req.params, &ctx).await {
        Ok(result) => Json(serde_json::json!({
            "ok": !result.is_error,
            "content": result.content
        })),
        Err(e) => Json(serde_json::json!({
            "error": { "message": format!("Tool error: {}", e) }
        })),
    }
}

// ─── Dashboard API Handlers ─────────────────────────────────────────────────────

async fn stats_handler(state: Arc<RwLock<GatewayState>>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let uptime = s.started_at.elapsed().as_secs();
    let sessions = s.session_store.stats().await;
    
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": uptime,
        "uptime_human": format_uptime(uptime),
        "connected_clients": s.presence.len(),
        "sessions": {
            "total": sessions.total_sessions,
            "messages": sessions.total_messages,
            "tokens": sessions.total_tokens
        },
        "gateway": {
            "host": s.config.gateway.host,
            "port": s.config.gateway.port,
            "auth_enabled": s.gateway_token.is_some()
        },
        "model": s.config.agents.defaults.model,
        "provider": s.config.agents.defaults.provider
    }))
}

async fn sessions_list_handler(state: Arc<RwLock<GatewayState>>) -> Json<serde_json::Value> {
    let s = state.read().await;
    // Return session list (simplified)
    Json(serde_json::json!({
        "sessions": [],
        "total": 0,
        "message": "Session list API - implement with session_store.list()"
    }))
}

async fn usage_handler(state: Arc<RwLock<GatewayState>>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let sessions = s.session_store.stats().await;
    
    Json(serde_json::json!({
        "tokens": {
            "input": 0,
            "output": 0,
            "total": sessions.total_tokens
        },
        "messages": sessions.total_messages,
        "requests": 0,
        "period": "all_time"
    }))
}

async fn config_handler(state: Arc<RwLock<GatewayState>>) -> Json<serde_json::Value> {
    let s = state.read().await;
    
    let telegram_enabled = match &s.config.channels.telegram {
        Some(t) => !t.bot_token.is_empty(),
        None => false,
    };
    let discord_enabled = match &s.config.channels.discord {
        Some(d) => !d.bot_token.is_empty(),
        None => false,
    };
    let slack_enabled = match &s.config.channels.slack {
        Some(sl) => sl.bot_token.is_some(),
        None => false,
    };
    
    // Return safe config (no secrets)
    Json(serde_json::json!({
        "gateway": {
            "host": s.config.gateway.host,
            "port": s.config.gateway.port,
            "verbose": s.config.gateway.verbose
        },
        "agents": {
            "model": s.config.agents.defaults.model,
            "provider": s.config.agents.defaults.provider,
            "workspace": s.config.agents.defaults.workspace
        },
        "channels": {
            "telegram": telegram_enabled,
            "discord": discord_enabled,
            "slack": slack_enabled
        },
        "tools": {
            "allow": s.config.tools.allow,
            "deny": s.config.tools.deny
        }
    }))
}

fn format_uptime(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    let secs = secs % 60;
    
    if days > 0 {
        format!("{}d {}h {}m", days, hours, mins)
    } else if hours > 0 {
        format!("{}h {}m {}s", hours, mins, secs)
    } else if mins > 0 {
        format!("{}m {}s", mins, secs)
    } else {
        format!("{}s", secs)
    }
}
