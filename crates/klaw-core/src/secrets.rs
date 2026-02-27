use std::collections::HashMap;
use std::path::PathBuf;
use tracing::info;

/// Simple secrets store — reads/writes ~/.klaw/secrets.json
/// (Plain-text for now; encryption can be added later)
pub struct SecretsStore {
    secrets: HashMap<String, String>,
    path: PathBuf,
}

impl SecretsStore {
    pub fn load() -> anyhow::Result<Self> {
        let path = crate::Config::home_dir().join("secrets.json");
        let secrets = if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            HashMap::new()
        };
        info!("Loaded {} secrets from {}", secrets.len(), path.display());
        Ok(Self { secrets, path })
    }

    pub fn save(&self) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&self.secrets)?;
        std::fs::write(&self.path, content)?;
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.secrets.get(key).map(|s| s.as_str())
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.secrets.insert(key.to_string(), value.to_string());
    }

    pub fn delete(&mut self, key: &str) -> bool {
        self.secrets.remove(key).is_some()
    }

    pub fn list(&self) -> Vec<&str> {
        self.secrets.keys().map(|k| k.as_str()).collect()
    }

    /// Resolve `${SECRET_NAME}` references in a string
    pub fn resolve(&self, input: &str) -> String {
        let mut result = input.to_string();
        for (key, value) in &self.secrets {
            let pattern = format!("${{{}}}", key);
            result = result.replace(&pattern, value);
        }
        result
    }
}
