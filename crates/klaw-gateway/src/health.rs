//! Health Check System
//! 
//! Service health monitoring and status reporting

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Health status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

impl Default for HealthStatus {
    fn default() -> Self {
        Self::Healthy
    }
}

/// Component health
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Component name
    pub name: String,
    /// Health status
    pub status: HealthStatus,
    /// Response time in ms
    pub response_time_ms: Option<u64>,
    /// Details
    pub details: HashMap<String, String>,
    /// Last check time
    pub last_check: Option<String>,
    /// Error message if unhealthy
    pub error: Option<String>,
}

impl ComponentHealth {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            status: HealthStatus::Healthy,
            response_time_ms: None,
            details: HashMap::new(),
            last_check: None,
            error: None,
        }
    }
    
    pub fn healthy(mut self) -> Self {
        self.status = HealthStatus::Healthy;
        self.error = None;
        self
    }
    
    pub fn degraded(mut self, reason: &str) -> Self {
        self.status = HealthStatus::Degraded;
        self.error = Some(reason.to_string());
        self
    }
    
    pub fn unhealthy(mut self, reason: &str) -> Self {
        self.status = HealthStatus::Unhealthy;
        self.error = Some(reason.to_string());
        self
    }
    
    pub fn with_response_time(mut self, ms: u64) -> Self {
        self.response_time_ms = Some(ms);
        self
    }
    
    pub fn with_detail(mut self, key: &str, value: &str) -> Self {
        self.details.insert(key.to_string(), value.to_string());
        self
    }
}

/// Health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    /// Overall status
    pub status: HealthStatus,
    /// Timestamp
    pub timestamp: String,
    /// Uptime in seconds
    pub uptime_seconds: u64,
    /// Version
    pub version: String,
    /// Components
    pub components: HashMap<String, ComponentHealth>,
}

/// Health checker
pub struct HealthChecker {
    start_time: Instant,
    version: String,
    checks: Vec<Box<dyn HealthCheck + Send + Sync>>,
}

/// Health check trait
#[async_trait::async_trait]
pub trait HealthCheck: Send + Sync {
    /// Component name
    fn name(&self) -> &str;
    
    /// Check health
    async fn check(&self) -> ComponentHealth;
}

impl HealthChecker {
    pub fn new(version: &str) -> Self {
        Self {
            start_time: Instant::now(),
            version: version.to_string(),
            checks: Vec::new(),
        }
    }
    
    /// Add health check
    pub fn add<C: HealthCheck + Send + Sync + 'static>(mut self, check: C) -> Self {
        self.checks.push(Box::new(check));
        self
    }
    
    /// Run all health checks
    pub async fn check(&self) -> HealthCheckResult {
        let mut components = HashMap::new();
        let mut overall_status = HealthStatus::Healthy;
        
        for check in &self.checks {
            let health = check.check().await;
            
            // Update overall status
            match health.status {
                HealthStatus::Unhealthy => overall_status = HealthStatus::Unhealthy,
                HealthStatus::Degraded if overall_status != HealthStatus::Unhealthy => {
                    overall_status = HealthStatus::Degraded;
                }
                _ => {}
            }
            
            components.insert(health.name.clone(), health);
        }
        
        HealthCheckResult {
            status: overall_status,
            timestamp: chrono::Utc::now().to_rfc3339(),
            uptime_seconds: self.start_time.elapsed().as_secs(),
            version: self.version.clone(),
            components,
        }
    }
}

/// Memory health check
pub struct MemoryHealthCheck {
    warn_threshold_mb: u64,
    critical_threshold_mb: u64,
}

impl MemoryHealthCheck {
    pub fn new() -> Self {
        Self {
            warn_threshold_mb: 512,
            critical_threshold_mb: 1024,
        }
    }
    
    pub fn with_thresholds(warn_mb: u64, critical_mb: u64) -> Self {
        Self {
            warn_threshold_mb: warn_mb,
            critical_threshold_mb: critical_mb,
        }
    }
}

impl Default for MemoryHealthCheck {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl HealthCheck for MemoryHealthCheck {
    fn name(&self) -> &str {
        "memory"
    }
    
    async fn check(&self) -> ComponentHealth {
        // Get approximate memory usage
        let usage = get_memory_usage_mb();
        
        let mut health = ComponentHealth::new("memory");
        
        match usage {
            Some(mb) => {
                health = health.with_detail("used_mb", &mb.to_string());
                health.response_time_ms = Some(0);
                
                if mb > self.critical_threshold_mb {
                    health = health.unhealthy(&format!("Memory critical: {}MB", mb));
                } else if mb > self.warn_threshold_mb {
                    health = health.degraded(&format!("Memory high: {}MB", mb));
                } else {
                    health = health.healthy();
                }
            }
            None => {
                health = health.with_detail("used_mb", "unknown");
            }
        }
        
        health
    }
}

fn get_memory_usage_mb() -> Option<u64> {
    // Approximate memory usage via stats
    // In production, use system-info crate for accurate values
    Some(std::mem::size_of::<u8>() as u64 * 1024 * 1024) // Placeholder
}

/// Simple health check (for testing)
pub struct SimpleHealthCheck {
    name: String,
    status: HealthStatus,
}

impl SimpleHealthCheck {
    pub fn healthy(name: &str) -> Self {
        Self {
            name: name.to_string(),
            status: HealthStatus::Healthy,
        }
    }
    
    pub fn unhealthy(name: &str) -> Self {
        Self {
            name: name.to_string(),
            status: HealthStatus::Unhealthy,
        }
    }
}

#[async_trait::async_trait]
impl HealthCheck for SimpleHealthCheck {
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn check(&self) -> ComponentHealth {
        match self.status {
            HealthStatus::Healthy => ComponentHealth::new(&self.name).healthy(),
            HealthStatus::Unhealthy => ComponentHealth::new(&self.name).unhealthy("Check failed"),
            HealthStatus::Degraded => ComponentHealth::new(&self.name).degraded("Degraded"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_health_status() {
        let mut result = ComponentHealth::new("test");
        assert_eq!(result.status, HealthStatus::Healthy);
        
        result = result.unhealthy("error");
        assert_eq!(result.status, HealthStatus::Unhealthy);
    }
    
    #[test]
    fn test_component_health() {
        let health = ComponentHealth::new("database")
            .healthy()
            .with_response_time(50)
            .with_detail("host", "localhost");
        
        assert_eq!(health.name, "database");
        assert!(health.response_time_ms.is_some());
        assert!(health.details.contains_key("host"));
    }
    
    #[tokio::test]
    async fn test_health_checker() {
        let checker = HealthChecker::new("1.0.0")
            .add(SimpleHealthCheck::healthy("db"))
            .add(SimpleHealthCheck::healthy("cache"));
        
        let result = checker.check().await;
        
        assert_eq!(result.status, HealthStatus::Healthy);
        assert_eq!(result.version, "1.0.0");
        assert!(result.components.contains_key("db"));
    }
    
    #[tokio::test]
    async fn test_health_checker_unhealthy() {
        let checker = HealthChecker::new("1.0.0")
            .add(SimpleHealthCheck::healthy("db"))
            .add(SimpleHealthCheck::unhealthy("cache"));
        
        let result = checker.check().await;
        
        // One unhealthy component makes overall status unhealthy
        assert_eq!(result.status, HealthStatus::Unhealthy);
    }
    
    #[tokio::test]
    async fn test_memory_health_check() {
        let check = MemoryHealthCheck::new();
        let health = check.check().await;
        
        assert_eq!(health.name, "memory");
    }
    
    #[test]
    fn test_uptime() {
        let checker = HealthChecker::new("1.0.0");
        std::thread::sleep(Duration::from_millis(100));
        
        assert!(checker.start_time.elapsed().as_millis() >= 100);
    }
}