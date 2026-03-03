//! Rate Limiting
//! 
//! Token bucket and sliding window rate limiting

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// Rate limit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RateLimitConfig {
    /// Max requests per window
    pub max_requests: u32,
    /// Window duration in seconds
    pub window_seconds: u64,
    /// Burst allowance
    pub burst: u32,
    /// Key prefix for tracking
    pub key_prefix: String,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 60,
            window_seconds: 60,
            burst: 10,
            key_prefix: "rate".to_string(),
        }
    }
}

impl RateLimitConfig {
    /// Create new rate limit config
    pub fn new(max_requests: u32, window_seconds: u64) -> Self {
        Self {
            max_requests,
            window_seconds,
            ..Default::default()
        }
    }
    
    /// Strict rate limit (10/min)
    pub fn strict() -> Self {
        Self {
            max_requests: 10,
            window_seconds: 60,
            burst: 0,
            ..Default::default()
        }
    }
    
    /// Relaxed rate limit (100/min)
    pub fn relaxed() -> Self {
        Self {
            max_requests: 100,
            window_seconds: 60,
            burst: 20,
            ..Default::default()
        }
    }
    
    /// API rate limit (1000/hour)
    pub fn api() -> Self {
        Self {
            max_requests: 1000,
            window_seconds: 3600,
            burst: 100,
            key_prefix: "api".to_string(),
        }
    }
}

/// Rate limit entry
#[derive(Debug, Clone)]
struct RateLimitEntry {
    requests: Vec<Instant>,
    burst_used: u32,
}

impl RateLimitEntry {
    fn new() -> Self {
        Self {
            requests: Vec::new(),
            burst_used: 0,
        }
    }
    
    fn cleanup(&mut self, window: Duration) {
        let cutoff = Instant::now() - window;
        self.requests.retain(|&t| t > cutoff);
    }
    
    fn count(&self) -> u32 {
        self.requests.len() as u32
    }
}

/// Rate limiter
pub struct RateLimiter {
    config: RateLimitConfig,
    entries: RwLock<HashMap<String, RateLimitEntry>>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            entries: RwLock::new(HashMap::new()),
        }
    }
    
    /// Check if request is allowed
    pub fn check(&self, key: &str) -> RateLimitResult {
        let full_key = format!("{}:{}", self.config.key_prefix, key);
        let window = Duration::from_secs(self.config.window_seconds);
        
        let mut entries = self.entries.write().unwrap();
        let entry = entries.entry(full_key).or_insert_with(RateLimitEntry::new);
        
        // Cleanup old requests
        entry.cleanup(window);
        
        let current = entry.count();
        let remaining = self.config.max_requests.saturating_sub(current);
        
        // Check if we're at limit
        if current >= self.config.max_requests {
            // Can we use burst?
            if entry.burst_used < self.config.burst {
                entry.burst_used += 1;
                entry.requests.push(Instant::now());
                let reset_after = if !entry.requests.is_empty() {
                    let oldest = entry.requests[0];
                    Some((oldest + window).duration_since(Instant::now()))
                } else {
                    None
                };
                return RateLimitResult {
                    allowed: true,
                    remaining: 0,
                    reset_after: reset_after,
                    retry_after: None,
                };
            }
            
            // Rate limited
            let reset_after = if !entry.requests.is_empty() {
                let oldest = entry.requests[0];
                Some((oldest + window).duration_since(Instant::now()))
            } else {
                None
            };
            
            return RateLimitResult {
                allowed: false,
                remaining: 0,
                reset_after: reset_after,
                retry_after: reset_after,
            };
        }
        
        // Allowed - add request
        entry.requests.push(Instant::now());
        
        RateLimitResult {
            allowed: true,
            remaining: remaining.saturating_sub(1),
            reset_after: None,
            retry_after: None,
        }
    }
    
    /// Reset rate limit for key
    pub fn reset(&self, key: &str) {
        let full_key = format!("{}:{}", self.config.key_prefix, key);
        let mut entries = self.entries.write().unwrap();
        entries.remove(&full_key);
    }
    
    /// Get remaining requests
    pub fn remaining(&self, key: &str) -> u32 {
        let full_key = format!("{}:{}", self.config.key_prefix, key);
        let window = Duration::from_secs(self.config.window_seconds);
        
        let entries = self.entries.read().unwrap();
        if let Some(entry) = entries.get(&full_key) {
            let count = entry.count();
            self.config.max_requests.saturating_sub(count)
        } else {
            self.config.max_requests
        }
    }
    
    /// Clean up all expired entries
    pub fn cleanup(&self) {
        let window = Duration::from_secs(self.config.window_seconds);
        let mut entries = self.entries.write().unwrap();
        
        entries.retain(|_, entry| {
            entry.cleanup(window);
            entry.count() > 0
        });
    }
    
    /// Get stats
    pub fn stats(&self) -> RateLimitStats {
        let entries = self.entries.read().unwrap();
        let total_keys = entries.len();
        let total_requests: u64 = entries.values()
            .map(|e| e.count() as u64)
            .sum();
        
        RateLimitStats {
            total_keys,
            total_requests,
            config: self.config.clone(),
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(RateLimitConfig::default())
    }
}

/// Rate limit check result
#[derive(Debug, Clone)]
pub struct RateLimitResult {
    pub allowed: bool,
    pub remaining: u32,
    pub reset_after: Option<Duration>,
    pub retry_after: Option<Duration>,
}

/// Rate limit stats
#[derive(Debug, Clone, Serialize)]
pub struct RateLimitStats {
    pub total_keys: usize,
    pub total_requests: u64,
    pub config: RateLimitConfig,
}

/// Multi-key rate limiter
pub struct MultiRateLimiter {
    limits: HashMap<String, RateLimiter>,
}

impl MultiRateLimiter {
    pub fn new() -> Self {
        Self {
            limits: HashMap::new(),
        }
    }
    
    /// Add rate limit
    pub fn add_limit(&mut self, name: &str, config: RateLimitConfig) {
        self.limits.insert(name.to_string(), RateLimiter::new(config));
    }
    
    /// Check all limits
    pub fn check_all(&self, key: &str) -> HashMap<String, RateLimitResult> {
        self.limits.iter()
            .map(|(name, limiter)| (name.clone(), limiter.check(key)))
            .collect()
    }
    
    /// Check if all allow
    pub fn is_allowed(&self, key: &str) -> bool {
        self.limits.values().all(|l| l.check(key).allowed)
    }
    
    /// Get most restrictive
    pub fn get_retry_after(&self, key: &str) -> Option<Duration> {
        self.limits.values()
            .filter_map(|l| l.check(key).retry_after)
            .max()
    }
}

impl Default for MultiRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_rate_limiter_basic() {
        let mut config = RateLimitConfig::new(5, 60);
        config.burst = 0; // No burst for test
        let limiter = RateLimiter::new(config);
        
        // Should allow first 5 requests
        for i in 0..5 {
            let result = limiter.check("test");
            assert!(result.allowed, "Request {} should be allowed", i);
        }
        
        // 6th should be denied (no burst)
        let result = limiter.check("test");
        assert!(!result.allowed, "6th request should be denied");
    }
    
    #[test]
    fn test_rate_limiter_remaining() {
        let config = RateLimitConfig::new(10, 60);
        let limiter = RateLimiter::new(config);
        
        assert_eq!(limiter.remaining("test"), 10);
        
        limiter.check("test");
        assert_eq!(limiter.remaining("test"), 9);
    }
    
    #[test]
    fn test_rate_limiter_reset() {
        let mut config = RateLimitConfig::new(2, 60);
        config.burst = 0; // No burst for test
        let limiter = RateLimiter::new(config);
        
        limiter.check("test");
        limiter.check("test");
        
        // Now we're at limit
        let result = limiter.check("test");
        assert!(!result.allowed);
        
        // Reset and try again
        limiter.reset("test");
        let result = limiter.check("test");
        assert!(result.allowed);
    }
    
    #[test]
    fn test_rate_limiter_different_keys() {
        let mut config = RateLimitConfig::new(2, 60);
        config.burst = 0; // No burst for test
        let limiter = RateLimiter::new(config);
        
        assert!(limiter.check("key1").allowed);
        assert!(limiter.check("key2").allowed);
        
        limiter.check("key1"); // key1 is now at 2
        
        // key1 should be blocked
        let result = limiter.check("key1");
        assert!(!result.allowed);
        
        // key2 should still work
        assert!(limiter.check("key2").allowed);
    }
    
    #[test]
    fn test_rate_limit_config_presets() {
        let strict = RateLimitConfig::strict();
        assert_eq!(strict.max_requests, 10);
        
        let relaxed = RateLimitConfig::relaxed();
        assert_eq!(relaxed.max_requests, 100);
        
        let api = RateLimitConfig::api();
        assert_eq!(api.max_requests, 1000);
    }
    
    #[test]
    fn test_multi_rate_limiter() {
        let mut multi = MultiRateLimiter::new();
        multi.add_limit("strict", RateLimitConfig::strict());
        multi.add_limit("relaxed", RateLimitConfig::relaxed());
        
        // First request should pass both
        assert!(multi.is_allowed("test"));
        
        // Exhaust strict limit (10 requests)
        for _ in 0..10 {
            multi.check_all("test");
        }
        
        // Should be blocked by strict
        assert!(!multi.is_allowed("test"));
    }
}