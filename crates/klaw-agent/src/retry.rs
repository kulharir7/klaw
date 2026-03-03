//! Retry logic for LLM API calls
//! Handles rate limits, timeouts, and transient errors

use std::time::Duration;
use std::future::Future;
use tracing::{info, warn};

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retries
    pub max_retries: u32,
    /// Initial delay in milliseconds
    pub initial_delay_ms: u64,
    /// Maximum delay in milliseconds
    pub max_delay_ms: u64,
    /// Backoff multiplier (exponential)
    pub backoff_multiplier: f64,
    /// Retry on these error types
    pub retry_on: Vec<RetryCondition>,
}

#[derive(Debug, Clone)]
pub enum RetryCondition {
    /// Rate limit (429)
    RateLimit,
    /// Timeout
    Timeout,
    /// Server error (5xx)
    ServerError,
    /// Network error
    NetworkError,
    /// Custom condition
    Custom(String),
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 30000,
            backoff_multiplier: 2.0,
            retry_on: vec![
                RetryCondition::RateLimit,
                RetryCondition::Timeout,
                RetryCondition::ServerError,
                RetryCondition::NetworkError,
            ],
        }
    }
}

impl RetryConfig {
    /// Create retry config with custom max retries
    pub fn with_max_retries(max: u32) -> Self {
        Self {
            max_retries: max,
            ..Self::default()
        }
    }
    
    /// Calculate delay for a given attempt
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let delay = self.initial_delay_ms as f64 
            * self.backoff_multiplier.powi(attempt as i32);
        let delay = delay.min(self.max_delay_ms as f64);
        Duration::from_millis(delay as u64)
    }
    
    /// Check if an error should trigger a retry
    pub fn should_retry(&self, error: &str, status_code: Option<u16>) -> bool {
        for condition in &self.retry_on {
            match condition {
                RetryCondition::RateLimit => {
                    if status_code == Some(429) {
                        return true;
                    }
                }
                RetryCondition::Timeout => {
                    if error.contains("timeout") || error.contains("Timeout") {
                        return true;
                    }
                }
                RetryCondition::ServerError => {
                    if let Some(code) = status_code {
                        if code >= 500 && code < 600 {
                            return true;
                        }
                    }
                }
                RetryCondition::NetworkError => {
                    if error.contains("connection") 
                        || error.contains("network")
                        || error.contains("ECONNREFUSED")
                        || error.contains("ENOTFOUND") {
                        return true;
                    }
                }
                RetryCondition::Custom(pattern) => {
                    if error.contains(pattern) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

/// Retry state tracker
#[derive(Debug, Clone)]
pub struct RetryState {
    pub attempt: u32,
    pub last_error: Option<String>,
    pub last_status: Option<u16>,
    pub total_delay_ms: u64,
}

impl RetryState {
    pub fn new() -> Self {
        Self {
            attempt: 0,
            last_error: None,
            last_status: None,
            total_delay_ms: 0,
        }
    }
    
    pub fn record_error(&mut self, error: &str, status: Option<u16>) {
        self.attempt += 1;
        self.last_error = Some(error.to_string());
        self.last_status = status;
    }
    
    pub fn should_continue(&self, config: &RetryConfig) -> bool {
        if self.attempt >= config.max_retries {
            return false;
        }
        if let Some(ref error) = self.last_error {
            config.should_retry(error, self.last_status)
        } else {
            false
        }
    }
}

impl Default for RetryState {
    fn default() -> Self {
        Self::new()
    }
}

/// Execute a function with retry logic
pub async fn with_retry<F, Fut, T, E>(
    config: &RetryConfig,
    mut f: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut state = RetryState::new();
    
    loop {
        match f().await {
            Ok(result) => {
                if state.attempt > 0 {
                    info!("Request succeeded after {} retries", state.attempt);
                }
                return Ok(result);
            }
            Err(e) => {
                let error_str = format!("{}", e);
                state.record_error(&error_str, None);
                
                if !state.should_continue(config) {
                    warn!("Request failed after {} attempts: {}", state.attempt, error_str);
                    return Err(e);
                }
                
                let delay = config.delay_for_attempt(state.attempt);
                state.total_delay_ms += delay.as_millis() as u64;
                
                info!(
                    "Request failed (attempt {}/{}), retrying in {:?}: {}",
                    state.attempt, config.max_retries, delay, error_str
                );
                
                tokio::time::sleep(delay).await;
            }
        }
    }
}

/// Execute a function with retry logic (with status code)
pub async fn with_retry_status<F, Fut, T>(
    config: &RetryConfig,
    mut f: F,
) -> Result<T, String>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, (String, Option<u16>)>>,
{
    let mut state = RetryState::new();
    
    loop {
        match f().await {
            Ok(result) => {
                if state.attempt > 0 {
                    info!("Request succeeded after {} retries", state.attempt);
                }
                return Ok(result);
            }
            Err((error, status)) => {
                state.record_error(&error, status);
                
                if !state.should_continue(config) {
                    warn!("Request failed after {} attempts: {}", state.attempt, error);
                    return Err(error);
                }
                
                let delay = config.delay_for_attempt(state.attempt);
                state.total_delay_ms += delay.as_millis() as u64;
                
                info!(
                    "Request failed with status {:?} (attempt {}/{}), retrying in {:?}",
                    status, state.attempt, config.max_retries, delay
                );
                
                tokio::time::sleep(delay).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay_ms, 1000);
        assert_eq!(config.max_delay_ms, 30000);
    }
    
    #[test]
    fn test_delay_calculation() {
        let config = RetryConfig::default();
        
        // First retry: 1000ms
        let d1 = config.delay_for_attempt(1);
        assert!(d1.as_millis() >= 1000);
        
        // Second retry: 2000ms
        let d2 = config.delay_for_attempt(2);
        assert!(d2.as_millis() >= 2000);
        
        // Third retry: 4000ms
        let d3 = config.delay_for_attempt(3);
        assert!(d3.as_millis() >= 4000);
    }
    
    #[test]
    fn test_should_retry_rate_limit() {
        let config = RetryConfig::default();
        
        assert!(config.should_retry("rate limit exceeded", Some(429)));
        assert!(config.should_retry("timeout", None));
        assert!(config.should_retry("server error", Some(500)));
        assert!(config.should_retry("connection refused", None));
        
        // Should not retry on client errors
        assert!(!config.should_retry("bad request", Some(400)));
        assert!(!config.should_retry("unauthorized", Some(401)));
    }
    
    #[test]
    fn test_retry_state() {
        let mut state = RetryState::new();
        let config = RetryConfig::default();
        
        assert_eq!(state.attempt, 0);
        
        state.record_error("timeout", None);
        assert_eq!(state.attempt, 1);
        assert!(state.should_continue(&config));
        
        state.record_error("timeout", None);
        state.record_error("timeout", None);
        assert_eq!(state.attempt, 3);
        assert!(!state.should_continue(&config));
    }
    
    #[test]
    fn test_custom_max_retries() {
        let config = RetryConfig::with_max_retries(5);
        assert_eq!(config.max_retries, 5);
    }
}