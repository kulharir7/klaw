use axum::{
    Router,
    body::Bytes,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    http::HeaderMap,
    response::{Html, IntoResponse, Json},
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
use tracing::{info, warn, error};

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

    // Start cron scheduler
    {
        tokio::spawn(async move {
            match crate::cron_scheduler::CronScheduler::load_jobs() {
                Ok(mut scheduler) => {
                    if !scheduler.job_count() == 0 {
                        info!("⏰ Cron scheduler started ({} jobs)", scheduler.job_count());
                        let (cron_tx, _cron_rx) = tokio::sync::mpsc::channel::<String>(16);
                        scheduler.run(cron_tx).await;
                    }
                }
                Err(e) => warn!("Cron scheduler load failed: {}", e),
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

                        let result = run_agent(
                            state_guard.provider.as_ref(),
                            &tools_reg,
                            &mut session,
                            user_msg,
                            &agent_config,
                            &tool_ctx,
                        ).await;

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
