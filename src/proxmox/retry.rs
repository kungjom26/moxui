//! Exponential backoff retry logic.

use std::time::Duration;
use tokio::time::sleep;

/// Retry policy.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of attempts (including the first).
    pub max_attempts: u32,
    /// Initial backoff duration.
    pub initial_backoff: Duration,
    /// Maximum backoff duration.
    pub max_backoff: Duration,
    /// Backoff multiplier (typically 2.0).
    pub multiplier: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(10),
            multiplier: 2.0,
        }
    }
}

impl RetryPolicy {
    /// Execute a future with retry logic.
    ///
    /// # Errors
    ///
    /// Returns the last error if all attempts fail.
    pub async fn execute<F, Fut, T, E>(&self, mut f: F) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        let mut backoff = self.initial_backoff;
        let mut last_err: Option<E> = None;

        for attempt in 0..self.max_attempts {
            match f().await {
                Ok(val) => return Ok(val),
                Err(e) => {
                    if attempt + 1 < self.max_attempts {
                        tracing::warn!(
                            attempt = attempt + 1,
                            max_attempts = self.max_attempts,
                            backoff_ms = backoff.as_millis() as u64,
                            error = %e,
                            "retrying after error"
                        );
                        sleep(backoff).await;
                        backoff = std::cmp::min(
                            Duration::from_secs_f64(backoff.as_secs_f64() * self.multiplier),
                            self.max_backoff,
                        );
                        last_err = Some(e);
                    } else {
                        last_err = Some(e);
                    }
                }
            }
        }

        Err(last_err.expect("at least one attempt"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_retry_succeeds_on_second_attempt() {
        let counter = AtomicU32::new(0);
        let policy = RetryPolicy {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_millis(10),
            multiplier: 2.0,
        };

        let result: Result<&str, &str> = policy
            .execute(|| {
                let count = counter.fetch_add(1, Ordering::SeqCst);
                async move {
                    if count == 0 {
                        Err("fail")
                    } else {
                        Ok("success")
                    }
                }
            })
            .await;

        assert_eq!(result.unwrap(), "success");
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
}
