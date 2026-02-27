use std::future::Future;
use std::time::Duration;
use tracing::{info, warn};

/// A chain of models + API keys to try on failure.
pub struct FailoverChain {
    pub models: Vec<String>,
    pub api_keys: Vec<String>,
    pub retry_count: u32,
    pub retry_delay: Duration,
}

impl FailoverChain {
    /// Build a chain from a primary model, optional failover list, and optional keys.
    pub fn new(
        primary: &str,
        failover: Option<&[String]>,
        keys: Option<&[String]>,
        retry_count: u32,
        retry_delay: Duration,
    ) -> Self {
        let mut models = vec![primary.to_string()];
        if let Some(extras) = failover {
            models.extend(extras.iter().cloned());
        }
        let api_keys = keys.map(|k| k.to_vec()).unwrap_or_default();
        Self {
            models,
            api_keys,
            retry_count,
            retry_delay,
        }
    }

    /// Returns `true` if the error looks retryable (rate-limit, auth, server error, timeout).
    fn is_retryable(err: &anyhow::Error) -> bool {
        let msg = err.to_string().to_lowercase();
        // HTTP status codes
        for code in ["429", "401", "403", "500", "502", "503"] {
            if msg.contains(code) {
                return true;
            }
        }
        // Common timeout / connection strings
        for pattern in ["timeout", "timed out", "connection refused", "rate limit"] {
            if msg.contains(pattern) {
                return true;
            }
        }
        false
    }

    /// Try each model (and rotate keys on 429), returning `(result, model_used)`.
    pub async fn execute<F, Fut, T>(&self, call: F) -> anyhow::Result<(T, String)>
    where
        F: Fn(&str, &str) -> Fut,
        Fut: Future<Output = anyhow::Result<T>>,
    {
        let default_key = String::new();
        let keys: &[String] = if self.api_keys.is_empty() {
            std::slice::from_ref(&default_key)
        } else {
            &self.api_keys
        };

        let mut last_err: Option<anyhow::Error> = None;

        for (model_idx, model) in self.models.iter().enumerate() {
            for (key_idx, key) in keys.iter().enumerate() {
                for attempt in 0..=self.retry_count {
                    if attempt > 0 {
                        info!(
                            model = %model,
                            attempt = attempt,
                            "Retrying after {}ms delay",
                            self.retry_delay.as_millis()
                        );
                        tokio::time::sleep(self.retry_delay).await;
                    }

                    info!(
                        model = %model,
                        key_index = key_idx,
                        attempt = attempt,
                        "Failover: trying model {}/{}",
                        model_idx + 1,
                        self.models.len()
                    );

                    match call(model, key).await {
                        Ok(result) => {
                            if model_idx > 0 || key_idx > 0 || attempt > 0 {
                                info!(model = %model, "Failover: succeeded with fallback model");
                            }
                            return Ok((result, model.clone()));
                        }
                        Err(e) => {
                            warn!(
                                model = %model,
                                error = %e,
                                "Failover: attempt failed"
                            );
                            if !Self::is_retryable(&e) {
                                last_err = Some(e);
                                // Non-retryable → skip retries, move to next model
                                break;
                            }
                            // On 429 specifically, try next key before retrying same key
                            let msg = e.to_string();
                            last_err = Some(e);
                            if msg.contains("429") && key_idx + 1 < keys.len() {
                                break; // move to next key
                            }
                        }
                    }
                }
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("All failover models exhausted")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_failover_primary_succeeds() {
        let chain = FailoverChain::new("model-a", None, None, 0, Duration::from_millis(0));
        let (result, model) = chain
            .execute(|m, _k| {
                let m = m.to_string();
                async move { Ok(format!("ok from {}", m)) }
            })
            .await
            .unwrap();
        assert_eq!(result, "ok from model-a");
        assert_eq!(model, "model-a");
    }

    #[tokio::test]
    async fn test_failover_falls_through() {
        let counter = AtomicUsize::new(0);
        let chain = FailoverChain::new(
            "model-a",
            Some(&["model-b".to_string()]),
            None,
            0,
            Duration::from_millis(0),
        );
        let (result, model) = chain
            .execute(|m, _k| {
                let m = m.to_string();
                let n = counter.fetch_add(1, Ordering::SeqCst);
                async move {
                    if n == 0 {
                        Err(anyhow::anyhow!("HTTP 429 rate limit"))
                    } else {
                        Ok(format!("ok from {}", m))
                    }
                }
            })
            .await
            .unwrap();
        assert_eq!(model, "model-b");
        assert!(result.contains("model-b"));
    }

    #[tokio::test]
    async fn test_failover_all_fail() {
        let chain = FailoverChain::new("model-a", None, None, 1, Duration::from_millis(0));
        let result: anyhow::Result<(String, String)> = chain
            .execute(|_m, _k| async move { Err(anyhow::anyhow!("HTTP 500 server error")) })
            .await;
        assert!(result.is_err());
    }
}
