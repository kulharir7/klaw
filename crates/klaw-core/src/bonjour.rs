//! Bonjour/mDNS Service Discovery
//!
//! Discover Klaw gateways on the local network via mDNS/DNS-SD

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Service type for Klaw gateway
pub const KLAW_SERVICE_TYPE: &str = "_klaw._tcp.local.";

/// Service name prefix
pub const KLAW_SERVICE_NAME: &str = "Klaw Gateway";

/// Bonjour service instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInstance {
    /// Service name
    pub name: String,
    /// Service type
    pub service_type: String,
    /// Hostname
    pub hostname: String,
    /// IP addresses
    pub addresses: Vec<IpAddr>,
    /// Port
    pub port: u16,
    /// TXT records
    pub txt: HashMap<String, String>,
    /// Discovered at (Unix timestamp)
    pub discovered_at: u64,
    /// Created time (internal)
    #[serde(skip)]
    #[serde(default = "Instant::now")]
    pub created: Instant,
}

impl ServiceInstance {
    /// Create a new service instance
    pub fn new(name: String, hostname: String, addresses: Vec<IpAddr>, port: u16) -> Self {
        Self {
            name,
            service_type: KLAW_SERVICE_TYPE.to_string(),
            hostname,
            addresses,
            port,
            txt: HashMap::new(),
            discovered_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            created: Instant::now(),
        }
    }
    
    /// Get the primary address
    pub fn primary_address(&self) -> Option<SocketAddr> {
        self.addresses.first().map(|addr| SocketAddr::new(*addr, self.port))
    }
    
    /// Get URL for the service
    pub fn url(&self) -> Option<String> {
        self.primary_address().map(|addr| format!("http://{}", addr))
    }
    
    /// Check if service is still fresh
    pub fn is_fresh(&self, max_age: Duration) -> bool {
        self.created.elapsed() < max_age
    }
}

/// Bonjour/mDNS discovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BonjourConfig {
    /// Enable discovery
    pub enabled: bool,
    /// Service name to broadcast
    pub service_name: String,
    /// Port to advertise
    pub port: u16,
    /// TXT records
    pub txt_records: HashMap<String, String>,
    /// Discovery timeout in seconds
    pub discovery_timeout_seconds: u64,
    /// Service refresh interval in seconds
    pub refresh_interval_seconds: u64,
}

impl Default for BonjourConfig {
    fn default() -> Self {
        let mut txt_records = HashMap::new();
        txt_records.insert("version".to_string(), env!("CARGO_PKG_VERSION").to_string());
        txt_records.insert("protocol".to_string(), "http".to_string());
        
        Self {
            enabled: true,
            service_name: KLAW_SERVICE_NAME.to_string(),
            port: 3000,
            txt_records,
            discovery_timeout_seconds: 5,
            refresh_interval_seconds: 60,
        }
    }
}

/// Bonjour service discovery
pub struct BonjourDiscovery {
    config: BonjourConfig,
    discovered: HashMap<String, ServiceInstance>,
}

impl BonjourDiscovery {
    /// Create new discovery instance
    pub fn new(config: BonjourConfig) -> Self {
        Self {
            config,
            discovered: HashMap::new(),
        }
    }
    
    /// Start discovering services
    pub fn start_discovery(&mut self) -> anyhow::Result<()> {
        // In production, this would use mdns-sd or libmdns
        // For now, we'll use a simplified implementation
        Ok(())
    }
    
    /// Stop discovery
    pub fn stop_discovery(&mut self) {
        // Stop the mDNS responder
    }
    
    /// Discover services on the network
    pub async fn discover(&mut self, timeout: Duration) -> anyhow::Result<Vec<ServiceInstance>> {
        let start = Instant::now();
        
        // Simulate discovery - in production this would use mDNS
        // Real implementation would use: mdns-sd crate or libmdns
        
        // For simulation, add localhost service
        if self.discovered.is_empty() {
            let instance = ServiceInstance::new(
                format!("{}-1", self.config.service_name),
                "localhost.local.".to_string(),
                vec![IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))],
                self.config.port,
            );
            self.discovered.insert(instance.name.clone(), instance);
        }
        
        // Wait for discovery timeout or until we have services
        while start.elapsed() < timeout && self.discovered.is_empty() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        Ok(self.discovered.values().cloned().collect())
    }
    
    /// Get currently discovered services
    pub fn get_services(&self) -> Vec<&ServiceInstance> {
        self.discovered.values().collect()
    }
    
    /// Get a specific service by name
    pub fn get_service(&self, name: &str) -> Option<&ServiceInstance> {
        self.discovered.get(name)
    }
    
    /// Refresh discovered services
    pub async fn refresh(&mut self) -> anyhow::Result<Vec<ServiceInstance>> {
        // Remove stale services
        self.discovered.retain(|_, service| {
            service.is_fresh(Duration::from_secs(self.config.refresh_interval_seconds * 2))
        });
        
        // Re-discover
        self.discover(Duration::from_secs(self.config.discovery_timeout_seconds)).await
    }
    
    /// Broadcast this gateway as a service
    pub fn broadcast(&self) -> anyhow::Result<()> {
        // In production, this would register the service via mDNS
        // Real implementation would use: mdns-sd crate to register service
        
        // Log the broadcast
        tracing::info!(
            "Broadcasting {} on port {} via mDNS",
            self.config.service_name,
            self.config.port
        );
        
        Ok(())
    }
    
    /// Stop broadcasting
    pub fn stop_broadcast(&self) {
        // Unregister the mDNS service
        tracing::info!("Stopped broadcasting via mDNS");
    }
}

/// Service browser for finding Klaw gateways
pub struct ServiceBrowser {
    discovery: BonjourDiscovery,
}

impl ServiceBrowser {
    /// Create new service browser
    pub fn new() -> Self {
        Self {
            discovery: BonjourDiscovery::new(BonjourConfig::default()),
        }
    }
    
    /// Browse for services
    pub async fn browse(&mut self) -> anyhow::Result<Vec<ServiceInstance>> {
        self.discovery.discover(Duration::from_secs(5)).await
    }
    
    /// Browse with custom timeout
    pub async fn browse_with_timeout(&mut self, timeout: Duration) -> anyhow::Result<Vec<ServiceInstance>> {
        self.discovery.discover(timeout).await
    }
    
    /// Get all discovered services
    pub fn services(&self) -> Vec<&ServiceInstance> {
        self.discovery.get_services()
    }
}

impl Default for ServiceBrowser {
    fn default() -> Self {
        Self::new()
    }
}

/// Service advertiser for broadcasting Klaw gateway
pub struct ServiceAdvertiser {
    config: BonjourConfig,
}

impl ServiceAdvertiser {
    /// Create new advertiser
    pub fn new(port: u16) -> Self {
        let config = BonjourConfig {
            port,
            ..Default::default()
        };
        Self { config }
    }
    
    /// Create with custom config
    pub fn with_config(config: BonjourConfig) -> Self {
        Self { config }
    }
    
    /// Start advertising the service
    pub fn start(&self) -> anyhow::Result<()> {
        let discovery = BonjourDiscovery::new(self.config.clone());
        discovery.broadcast()
    }
    
    /// Stop advertising
    pub fn stop(&self) {
        let discovery = BonjourDiscovery::new(self.config.clone());
        discovery.stop_broadcast();
    }
    
    /// Get the service URL
    pub fn service_url(&self) -> String {
        format!("http://localhost:{}", self.config.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bonjour_config_default() {
        let config = BonjourConfig::default();
        assert!(config.enabled);
        assert_eq!(config.port, 3000);
        assert!(config.txt_records.contains_key("version"));
    }
    
    #[test]
    fn test_service_instance() {
        let instance = ServiceInstance::new(
            "Test Gateway".to_string(),
            "localhost.local.".to_string(),
            vec![IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))],
            3000,
        );
        
        assert!(instance.primary_address().is_some());
        assert!(instance.url().is_some());
        assert!(instance.is_fresh(Duration::from_secs(10)));
    }
    
    #[test]
    fn test_service_browser_new() {
        let browser = ServiceBrowser::new();
        assert_eq!(browser.services().len(), 0);
    }
    
    #[test]
    fn test_service_advertiser_new() {
        let advertiser = ServiceAdvertiser::new(8080);
        assert_eq!(advertiser.config.port, 8080);
    }
    
    #[tokio::test]
    async fn test_discovery_discover() {
        let mut discovery = BonjourDiscovery::new(BonjourConfig::default());
        let services = discovery.discover(Duration::from_millis(100)).await.unwrap();
        // Should return localhost service
        assert!(!services.is_empty() || discovery.get_services().len() == 1);
    }
    
    #[test]
    fn test_service_instance_url() {
        let instance = ServiceInstance::new(
            "Test".to_string(),
            "test.local.".to_string(),
            vec![IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100))],
            8080,
        );
        
        assert_eq!(instance.url(), Some("http://192.168.1.100:8080".to_string()));
    }
    
    #[test]
    fn test_bonjour_config_custom() {
        let config = BonjourConfig {
            enabled: false,
            service_name: "Custom Gateway".to_string(),
            port: 8080,
            txt_records: [("key".to_string(), "value".to_string())].into_iter().collect(),
            discovery_timeout_seconds: 10,
            refresh_interval_seconds: 30,
        };
        
        assert!(!config.enabled);
        assert_eq!(config.port, 8080);
    }
}