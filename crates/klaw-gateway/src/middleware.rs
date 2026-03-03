//! Request Middleware
//! 
//! Middleware chain for request processing

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// Request context
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// Request ID
    pub request_id: String,
    /// User ID (if authenticated)
    pub user_id: Option<String>,
    /// Session key
    pub session_key: Option<String>,
    /// Agent ID
    pub agent_id: Option<String>,
    /// Channel
    pub channel: Option<String>,
    /// Metadata
    pub metadata: HashMap<String, String>,
    /// Start time
    pub start_time: std::time::Instant,
}

impl RequestContext {
    pub fn new() -> Self {
        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            user_id: None,
            session_key: None,
            agent_id: None,
            channel: None,
            metadata: HashMap::new(),
            start_time: std::time::Instant::now(),
        }
    }
    
    pub fn with_user(mut self, user_id: &str) -> Self {
        self.user_id = Some(user_id.to_string());
        self
    }
    
    pub fn with_session(mut self, session_key: &str) -> Self {
        self.session_key = Some(session_key.to_string());
        self
    }
    
    pub fn with_agent(mut self, agent_id: &str) -> Self {
        self.agent_id = Some(agent_id.to_string());
        self
    }
    
    pub fn with_channel(mut self, channel: &str) -> Self {
        self.channel = Some(channel.to_string());
        self
    }
    
    pub fn elapsed_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }
}

impl Default for RequestContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Middleware trait
#[async_trait]
pub trait Middleware: Send + Sync {
    /// Middleware name
    fn name(&self) -> &str;
    
    /// Process request (before handler)
    async fn before(&self, ctx: &mut RequestContext) -> Result<(), MiddlewareError>;
    
    /// Process response (after handler)
    async fn after(&self, ctx: &RequestContext, result: &MiddlewareResult) -> Result<(), MiddlewareError>;
}

/// Middleware result
#[derive(Debug, Clone)]
pub enum MiddlewareResult {
    /// Continue to next middleware
    Continue,
    /// Stop with response
    Response(String),
    /// Error occurred
    Error(String),
}

/// Middleware error
#[derive(Debug, Clone)]
pub enum MiddlewareError {
    /// Unauthorized
    Unauthorized(String),
    /// Forbidden
    Forbidden(String),
    /// Not found
    NotFound(String),
    /// Bad request
    BadRequest(String),
    /// Rate limited
    RateLimited(String),
    /// Internal error
    Internal(String),
}

impl std::fmt::Display for MiddlewareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MiddlewareError::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
            MiddlewareError::Forbidden(msg) => write!(f, "Forbidden: {}", msg),
            MiddlewareError::NotFound(msg) => write!(f, "Not found: {}", msg),
            MiddlewareError::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            MiddlewareError::RateLimited(msg) => write!(f, "Rate limited: {}", msg),
            MiddlewareError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for MiddlewareError {}

/// Middleware chain
pub struct MiddlewareChain {
    middlewares: Vec<Arc<dyn Middleware>>,
}

impl MiddlewareChain {
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }
    
    /// Add middleware
    pub fn add<M: Middleware + 'static>(mut self, middleware: M) -> Self {
        self.middlewares.push(Arc::new(middleware));
        self
    }
    
    /// Run before hooks
    pub async fn run_before(&self, ctx: &mut RequestContext) -> Result<(), MiddlewareError> {
        for middleware in &self.middlewares {
            middleware.before(ctx).await?;
        }
        Ok(())
    }
    
    /// Run after hooks
    pub async fn run_after(&self, ctx: &RequestContext, result: &MiddlewareResult) -> Result<(), MiddlewareError> {
        for middleware in self.middlewares.iter().rev() {
            middleware.after(ctx, result).await?;
        }
        Ok(())
    }
}

impl Default for MiddlewareChain {
    fn default() -> Self {
        Self::new()
    }
}

/// Logging middleware
pub struct LoggingMiddleware;

#[async_trait]
impl Middleware for LoggingMiddleware {
    fn name(&self) -> &str {
        "logging"
    }
    
    async fn before(&self, ctx: &mut RequestContext) -> Result<(), MiddlewareError> {
        tracing::info!(
            request_id = %ctx.request_id,
            user_id = ?ctx.user_id,
            channel = ?ctx.channel,
            "Request started"
        );
        Ok(())
    }
    
    async fn after(&self, ctx: &RequestContext, result: &MiddlewareResult) -> Result<(), MiddlewareError> {
        tracing::info!(
            request_id = %ctx.request_id,
            elapsed_ms = ctx.elapsed_ms(),
            result = ?result,
            "Request completed"
        );
        Ok(())
    }
}

/// Timing middleware
pub struct TimingMiddleware {
    warn_threshold_ms: u64,
}

impl TimingMiddleware {
    pub fn new(warn_threshold_ms: u64) -> Self {
        Self { warn_threshold_ms }
    }
}

#[async_trait]
impl Middleware for TimingMiddleware {
    fn name(&self) -> &str {
        "timing"
    }
    
    async fn before(&self, _ctx: &mut RequestContext) -> Result<(), MiddlewareError> {
        Ok(())
    }
    
    async fn after(&self, ctx: &RequestContext, _result: &MiddlewareResult) -> Result<(), MiddlewareError> {
        let elapsed = ctx.elapsed_ms();
        if elapsed > self.warn_threshold_ms {
            tracing::warn!(
                request_id = %ctx.request_id,
                elapsed_ms = elapsed,
                threshold_ms = self.warn_threshold_ms,
                "Slow request detected"
            );
        }
        Ok(())
    }
}

impl Default for TimingMiddleware {
    fn default() -> Self {
        Self::new(1000) // 1 second default threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_request_context() {
        let ctx = RequestContext::new()
            .with_user("user-123")
            .with_agent("agent-1")
            .with_channel("telegram");
        
        assert_eq!(ctx.user_id, Some("user-123".to_string()));
        assert_eq!(ctx.agent_id, Some("agent-1".to_string()));
        assert_eq!(ctx.channel, Some("telegram".to_string()));
    }
    
    #[test]
    fn test_elapsed_time() {
        let ctx = RequestContext::new();
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(ctx.elapsed_ms() >= 10);
    }
    
    #[test]
    fn test_middleware_chain() {
        let chain = MiddlewareChain::new()
            .add(LoggingMiddleware)
            .add(TimingMiddleware::new(1000));
        
        assert_eq!(chain.middlewares.len(), 2);
    }
    
    #[tokio::test]
    async fn test_logging_middleware() {
        let middleware = LoggingMiddleware;
        let mut ctx = RequestContext::new();
        
        let result = middleware.before(&mut ctx).await;
        assert!(result.is_ok());
        
        let result = middleware.after(&ctx, &MiddlewareResult::Continue).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_timing_middleware() {
        let middleware = TimingMiddleware::new(1000);
        let mut ctx = RequestContext::new();
        
        let result = middleware.before(&mut ctx).await;
        assert!(result.is_ok());
        
        let result = middleware.after(&ctx, &MiddlewareResult::Continue).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_middleware_chain_run() {
        let chain = MiddlewareChain::new()
            .add(LoggingMiddleware)
            .add(TimingMiddleware::default());
        
        let mut ctx = RequestContext::new();
        let result = chain.run_before(&mut ctx).await;
        assert!(result.is_ok());
        
        let result = chain.run_after(&ctx, &MiddlewareResult::Continue).await;
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_middleware_error() {
        let err = MiddlewareError::Unauthorized("test".to_string());
        assert!(err.to_string().contains("Unauthorized"));
        
        let err = MiddlewareError::RateLimited("too many".to_string());
        assert!(err.to_string().contains("Rate limited"));
    }
}