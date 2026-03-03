use std::collections::HashMap;
use std::path::PathBuf;
use tracing::info;
use serde::{Deserialize, Serialize};

/// Secrets store with optional encryption
/// Stores secrets in ~/.klaw/secrets.json
/// When encryption is enabled, values are encrypted with a master key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsStore {
    /// Map of key -> encrypted_value (or plaintext if not encrypted)
    secrets: HashMap<String, String>,
    /// Whether encryption is enabled
    #[serde(default)]
    encrypted: bool,
    /// Path to the secrets file
    #[serde(skip)]
    path: PathBuf,
    /// Master key for encryption (loaded from environment or file)
    #[serde(skip)]
    master_key: Option<Vec<u8>>,
}

impl SecretsStore {
    /// Load secrets from ~/.klaw/secrets.json
    pub fn load() -> anyhow::Result<Self> {
        let path = crate::Config::home_dir().join("secrets.json");
        Self::load_from(&path)
    }
    
    /// Load secrets from a specific path
    pub fn load_from(path: &PathBuf) -> anyhow::Result<Self> {
        let secrets = if path.exists() {
            let content = std::fs::read_to_string(path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            HashMap::new()
        };
        
        // Check for master key from environment
        let master_key = std::env::var("KLAW_MASTER_KEY")
            .ok()
            .map(|k| k.into_bytes());
        
        // Check if encrypted by looking at the file
        let encrypted = path.exists() && {
            let content = std::fs::read_to_string(path)?;
            let json: serde_json::Value = serde_json::from_str(&content)?;
            json.get("encrypted").and_then(|v| v.as_bool()).unwrap_or(false)
        };
        
        info!("Loaded {} secrets from {} (encrypted: {})", secrets.len(), path.display(), encrypted);
        
        Ok(Self {
            secrets,
            encrypted,
            path: path.clone(),
            master_key,
        })
    }
    
    /// Enable encryption with a master key
    pub fn enable_encryption(&mut self, master_key: &[u8]) {
        self.master_key = Some(master_key.to_vec());
        self.encrypted = true;
    }
    
    /// Disable encryption (decrypts all values if key is available)
    pub fn disable_encryption(&mut self) -> anyhow::Result<()> {
        if !self.encrypted {
            return Ok(());
        }
        
        // Decrypt all values
        for (_key, value) in self.secrets.iter_mut() {
            if let Some(ref master_key) = self.master_key {
                *value = Self::decrypt(value, master_key)?;
            }
        }
        
        self.encrypted = false;
        Ok(())
    }

    pub fn save(&self) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        #[derive(Serialize, Deserialize)]
        struct SecretsFile {
            secrets: HashMap<String, String>,
            encrypted: bool,
        }
        
        let file = SecretsFile {
            secrets: self.secrets.clone(),
            encrypted: self.encrypted,
        };
        
        let content = serde_json::to_string_pretty(&file)?;
        std::fs::write(&self.path, content)?;
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.secrets.get(key).map(|s| {
            if self.encrypted {
                if let Some(ref master_key) = self.master_key {
                    Self::decrypt(s, master_key).unwrap_or_else(|_| s.clone())
                } else {
                    s.clone() // Can't decrypt without key
                }
            } else {
                s.clone()
            }
        })
    }
    
    /// Get raw value (encrypted if stored encrypted)
    pub fn get_raw(&self, key: &str) -> Option<&str> {
        self.secrets.get(key).map(|s| s.as_str())
    }

    pub fn set(&mut self, key: &str, value: &str) {
        let value = if self.encrypted {
            if let Some(ref master_key) = self.master_key {
                Self::encrypt(value, master_key)
            } else {
                value.to_string()
            }
        } else {
            value.to_string()
        };
        self.secrets.insert(key.to_string(), value);
    }

    pub fn delete(&mut self, key: &str) -> bool {
        self.secrets.remove(key).is_some()
    }

    pub fn list(&self) -> Vec<&str> {
        self.secrets.keys().map(|k| k.as_str()).collect()
    }
    
    /// Check if encryption is enabled
    pub fn is_encrypted(&self) -> bool {
        self.encrypted
    }
    
    /// Count of secrets
    pub fn len(&self) -> usize {
        self.secrets.len()
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.secrets.is_empty()
    }

    /// Resolve `${SECRET_NAME}` references in a string
    pub fn resolve(&self, input: &str) -> String {
        let mut result = input.to_string();
        for (key, _) in &self.secrets {
            let pattern = format!("${{{}}}", key);
            if let Some(value) = self.get(key) {
                result = result.replace(&pattern, &value);
            }
        }
        result
    }
    
    /// Simple XOR-based encryption (for demo - use proper encryption in production)
    fn encrypt(plaintext: &str, key: &[u8]) -> String {
        let bytes: Vec<u8> = plaintext.bytes()
            .enumerate()
            .map(|(i, b)| b ^ key[i % key.len()])
            .collect();
        base64_encode(&bytes)
    }
    
    /// Simple XOR-based decryption
    fn decrypt(ciphertext: &str, key: &[u8]) -> anyhow::Result<String> {
        let bytes = base64_decode(ciphertext)?;
        let decrypted: Vec<u8> = bytes.iter()
            .enumerate()
            .map(|(i, b)| b ^ key[i % key.len()])
            .collect();
        Ok(String::from_utf8(decrypted)?)
    }
    
    /// Generate a new master key
    pub fn generate_master_key() -> Vec<u8> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let random_bytes: Vec<u8> = (0..32).map(|i| {
            let b = ((timestamp >> (i % 8)) & 0xFF) as u8;
            b.wrapping_add(i as u8)
        }).collect();
        random_bytes
    }
}

// Base64 encoding/decoding helpers
fn base64_encode(data: &[u8]) -> String {
    use base64::{Engine, engine::general_purpose::STANDARD};
    STANDARD.encode(data)
}

fn base64_decode(s: &str) -> anyhow::Result<Vec<u8>> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    Ok(STANDARD.decode(s)?)
}

impl Default for SecretsStore {
    fn default() -> Self {
        Self::load().unwrap_or_else(|_| Self {
            secrets: HashMap::new(),
            encrypted: false,
            path: crate::Config::home_dir().join("secrets.json"),
            master_key: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encrypt_decrypt() {
        let key = b"test-master-key-32-bytes-long!!!!";
        let plaintext = "my-secret-api-key";
        let encrypted = SecretsStore::encrypt(plaintext, key);
        let decrypted = SecretsStore::decrypt(&encrypted, key).unwrap();
        assert_eq!(decrypted, plaintext);
    }
    
    #[test]
    fn test_secret_store() {
        let mut store = SecretsStore::default();
        store.set("API_KEY", "sk-test-123");
        store.set("DB_PASSWORD", "secret123");
        
        assert_eq!(store.get("API_KEY"), Some("sk-test-123".to_string()));
        assert_eq!(store.get("DB_PASSWORD"), Some("secret123".to_string()));
        assert!(store.get("NONEXISTENT").is_none());
    }
    
    #[test]
    fn test_resolve() {
        let mut store = SecretsStore::default();
        store.set("API_KEY", "sk-test-123");
        
        let input = "The key is ${API_KEY}";
        let resolved = store.resolve(input);
        assert_eq!(resolved, "The key is sk-test-123");
    }
    
    #[test]
    fn test_encrypted_store() {
        let master_key = SecretsStore::generate_master_key();
        let mut store = SecretsStore::default();
        store.enable_encryption(&master_key);
        
        store.set("SECRET", "my-secret-value");
        
        // Raw value should be encrypted
        let raw = store.get_raw("SECRET").unwrap();
        assert_ne!(raw, "my-secret-value");
        
        // Get should decrypt
        let decrypted = store.get("SECRET").unwrap();
        assert_eq!(decrypted, "my-secret-value");
    }
}
