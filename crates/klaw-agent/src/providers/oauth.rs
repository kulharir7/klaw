use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{info, warn};

/// Stored OAuth token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    pub provider: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub token_type: String,
    pub scope: Option<String>,
}

impl OAuthToken {
    pub fn is_expired(&self) -> bool {
        if let Some(exp) = self.expires_at {
            chrono::Utc::now().timestamp() >= exp - 60 // 60s buffer
        } else {
            false
        }
    }
}

/// OAuth provider configuration
#[derive(Debug, Clone)]
pub struct OAuthProviderConfig {
    pub name: String,
    pub auth_url: String,
    pub token_url: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub scopes: Vec<String>,
    pub device_code_url: Option<String>, // For device code flow
}

/// OAuth flow type
#[derive(Debug, Clone)]
pub enum OAuthFlow {
    /// Browser-based authorization code flow
    AuthorizationCode,
    /// Device code flow (for CLI — user goes to URL, enters code)
    DeviceCode,
}

/// Built-in OAuth provider configs
pub fn oauth_providers() -> HashMap<String, OAuthProviderConfig> {
    let mut p = HashMap::new();

    // Google Antigravity (Gemini OAuth)
    p.insert("google-antigravity".into(), OAuthProviderConfig {
        name: "Google Antigravity".into(),
        auth_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
        token_url: "https://oauth2.googleapis.com/token".into(),
        client_id: String::new(), // User must provide or use bundled plugin
        client_secret: None,
        scopes: vec!["https://www.googleapis.com/auth/cloud-platform".into()],
        device_code_url: Some("https://oauth2.googleapis.com/device/code".into()),
    });

    // Google Gemini CLI OAuth
    p.insert("google-gemini-cli".into(), OAuthProviderConfig {
        name: "Google Gemini CLI".into(),
        auth_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
        token_url: "https://oauth2.googleapis.com/token".into(),
        client_id: String::new(),
        client_secret: None,
        scopes: vec!["https://www.googleapis.com/auth/generative-language".into()],
        device_code_url: Some("https://oauth2.googleapis.com/device/code".into()),
    });

    // Qwen Portal OAuth (device code flow)
    p.insert("qwen-portal".into(), OAuthProviderConfig {
        name: "Qwen Portal".into(),
        auth_url: "https://auth.qwen.ai/oauth2/authorize".into(),
        token_url: "https://auth.qwen.ai/oauth2/token".into(),
        client_id: String::new(),
        client_secret: None,
        scopes: vec!["model:read".into()],
        device_code_url: Some("https://auth.qwen.ai/oauth2/device/code".into()),
    });

    // OpenAI Codex OAuth
    p.insert("openai-codex".into(), OAuthProviderConfig {
        name: "OpenAI Codex".into(),
        auth_url: "https://auth0.openai.com/authorize".into(),
        token_url: "https://auth0.openai.com/oauth/token".into(),
        client_id: String::new(),
        client_secret: None,
        scopes: vec!["openid profile email offline_access".into()],
        device_code_url: None,
    });

    // Anthropic Max/Pro (Claude subscription)
    p.insert("anthropic-max".into(), OAuthProviderConfig {
        name: "Anthropic Max".into(),
        auth_url: "https://console.anthropic.com/oauth/authorize".into(),
        token_url: "https://console.anthropic.com/oauth/token".into(),
        client_id: String::new(),
        client_secret: None,
        scopes: vec!["api:access".into()],
        device_code_url: None,
    });

    p
}

/// Token store — persists OAuth tokens to disk
pub struct TokenStore {
    store_dir: PathBuf,
    tokens: HashMap<String, OAuthToken>,
}

impl TokenStore {
    pub fn new() -> Self {
        let store_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".klaw")
            .join("auth");

        let mut store = Self {
            store_dir,
            tokens: HashMap::new(),
        };
        store.load();
        store
    }

    /// Load tokens from disk
    fn load(&mut self) {
        let tokens_file = self.store_dir.join("tokens.json");
        if tokens_file.exists() {
            match std::fs::read_to_string(&tokens_file) {
                Ok(content) => {
                    match serde_json::from_str::<HashMap<String, OAuthToken>>(&content) {
                        Ok(tokens) => {
                            info!("Loaded {} OAuth tokens", tokens.len());
                            self.tokens = tokens;
                        }
                        Err(e) => warn!("Failed to parse tokens: {}", e),
                    }
                }
                Err(e) => warn!("Failed to read tokens: {}", e),
            }
        }
    }

    /// Save tokens to disk
    pub fn save(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.store_dir)?;
        let tokens_file = self.store_dir.join("tokens.json");
        let content = serde_json::to_string_pretty(&self.tokens)?;
        std::fs::write(&tokens_file, content)?;
        Ok(())
    }

    /// Get a token for a provider
    pub fn get(&self, provider: &str) -> Option<&OAuthToken> {
        self.tokens.get(provider)
    }

    /// Store a token
    pub fn set(&mut self, provider: &str, token: OAuthToken) {
        self.tokens.insert(provider.to_string(), token);
        let _ = self.save();
    }

    /// Remove a token
    pub fn remove(&mut self, provider: &str) {
        self.tokens.remove(provider);
        let _ = self.save();
    }

    /// List stored providers
    pub fn list(&self) -> Vec<String> {
        self.tokens.keys().cloned().collect()
    }

    /// Check if a valid (non-expired) token exists
    pub fn has_valid_token(&self, provider: &str) -> bool {
        self.tokens.get(provider).map(|t| !t.is_expired()).unwrap_or(false)
    }
}

/// Device code flow — step 1: request device code
pub async fn request_device_code(
    config: &OAuthProviderConfig,
) -> anyhow::Result<DeviceCodeResponse> {
    let device_url = config.device_code_url.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Provider {} doesn't support device code flow", config.name))?;

    let client = reqwest::Client::new();
    let response = client.post(device_url)
        .form(&[
            ("client_id", config.client_id.as_str()),
            ("scope", &config.scopes.join(" ")),
        ])
        .send()
        .await?;

    let data: DeviceCodeResponse = response.json().await?;
    Ok(data)
}

/// Device code flow — step 2: poll for token
pub async fn poll_device_token(
    config: &OAuthProviderConfig,
    device_code: &str,
    interval: u64,
) -> anyhow::Result<OAuthToken> {
    let client = reqwest::Client::new();

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(interval)).await;

        let response = client.post(&config.token_url)
            .form(&[
                ("client_id", config.client_id.as_str()),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await?;

        let data: serde_json::Value = response.json().await?;

        if let Some(error) = data.get("error").and_then(|e| e.as_str()) {
            match error {
                "authorization_pending" => continue,
                "slow_down" => {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
                "expired_token" => anyhow::bail!("Device code expired. Please try again."),
                "access_denied" => anyhow::bail!("Access denied by user."),
                _ => anyhow::bail!("OAuth error: {}", error),
            }
        }

        let token = OAuthToken {
            provider: config.name.clone(),
            access_token: data["access_token"].as_str().unwrap_or("").to_string(),
            refresh_token: data["refresh_token"].as_str().map(|s| s.to_string()),
            expires_at: data["expires_in"].as_i64().map(|e| chrono::Utc::now().timestamp() + e),
            token_type: data["token_type"].as_str().unwrap_or("Bearer").to_string(),
            scope: data["scope"].as_str().map(|s| s.to_string()),
        };

        return Ok(token);
    }
}

/// Refresh an expired token
pub async fn refresh_token(
    config: &OAuthProviderConfig,
    token: &OAuthToken,
) -> anyhow::Result<OAuthToken> {
    let refresh_token = token.refresh_token.as_ref()
        .ok_or_else(|| anyhow::anyhow!("No refresh token available"))?;

    let client = reqwest::Client::new();
    let mut form = vec![
        ("client_id", config.client_id.as_str()),
        ("refresh_token", refresh_token.as_str()),
        ("grant_type", "refresh_token"),
    ];
    if let Some(ref secret) = config.client_secret {
        form.push(("client_secret", secret.as_str()));
    }

    let response = client.post(&config.token_url)
        .form(&form)
        .send()
        .await?;

    if !response.status().is_success() {
        let error = response.text().await.unwrap_or_default();
        anyhow::bail!("Token refresh failed: {}", error);
    }

    let data: serde_json::Value = response.json().await?;

    Ok(OAuthToken {
        provider: token.provider.clone(),
        access_token: data["access_token"].as_str().unwrap_or("").to_string(),
        refresh_token: data["refresh_token"].as_str().map(|s| s.to_string()).or(token.refresh_token.clone()),
        expires_at: data["expires_in"].as_i64().map(|e| chrono::Utc::now().timestamp() + e),
        token_type: data["token_type"].as_str().unwrap_or("Bearer").to_string(),
        scope: data["scope"].as_str().map(|s| s.to_string()).or(token.scope.clone()),
    })
}

/// Authorization code flow — step 1: generate auth URL
pub fn auth_code_url(config: &OAuthProviderConfig, redirect_uri: &str, state: &str) -> String {
    format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
        config.auth_url,
        urlencoding::encode(&config.client_id),
        urlencoding::encode(redirect_uri),
        urlencoding::encode(&config.scopes.join(" ")),
        urlencoding::encode(state),
    )
}

/// Authorization code flow — step 2: exchange code for token
pub async fn exchange_code(
    config: &OAuthProviderConfig,
    code: &str,
    redirect_uri: &str,
) -> anyhow::Result<OAuthToken> {
    let client = reqwest::Client::new();
    let mut form = vec![
        ("client_id", config.client_id.as_str()),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("grant_type", "authorization_code"),
    ];
    if let Some(ref secret) = config.client_secret {
        form.push(("client_secret", secret.as_str()));
    }

    let response = client.post(&config.token_url)
        .form(&form)
        .send()
        .await?;

    if !response.status().is_success() {
        let error = response.text().await.unwrap_or_default();
        anyhow::bail!("Code exchange failed: {}", error);
    }

    let data: serde_json::Value = response.json().await?;

    Ok(OAuthToken {
        provider: config.name.clone(),
        access_token: data["access_token"].as_str().unwrap_or("").to_string(),
        refresh_token: data["refresh_token"].as_str().map(|s| s.to_string()),
        expires_at: data["expires_in"].as_i64().map(|e| chrono::Utc::now().timestamp() + e),
        token_type: data["token_type"].as_str().unwrap_or("Bearer").to_string(),
        scope: data["scope"].as_str().map(|s| s.to_string()),
    })
}

#[derive(Debug, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub expires_in: u64,
    pub interval: Option<u64>,
}

/// URL encoding helper
mod urlencoding {
    pub fn encode(s: &str) -> String {
        s.chars().map(|c| {
            match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
                _ => format!("%{:02X}", c as u8),
            }
        }).collect()
    }
}
