//! Circuit breaker for Proxmox API calls.
//!
//! Opens after N consecutive failures, blocks all calls for cooldown period,
//! then enters half-open state to test recovery.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit is closed (normal operation).
    Closed,
    /// Circuit is open (failing, reject calls).
    Open,
    /// Circuit is half-open (testing recovery).
    HalfOpen,
}

/// Circuit breaker for one Proxmox cluster.
pub struct CircuitBreaker {
    /// Failure threshold to open circuit.
    failure_threshold: u32,
    /// Cooldown duration before half-open.
    cooldown: Duration,
    /// Current consecutive failure count.
    failure_count: AtomicU32,
    /// Unix timestamp when circuit opened (0 = never).
    opened_at: AtomicU64,
}

impl CircuitBreaker {
    /// Create a new circuit breaker.
    pub fn new(failure_threshold: u32, cooldown: Duration) -> Self {
        Self {
            failure_threshold,
            cooldown,
            failure_count: AtomicU32::new(0),
            opened_at: AtomicU64::new(0),
        }
    }

    /// Get current circuit state.
    pub fn state(&self) -> CircuitState {
        match self.opened_at.load(Ordering::Relaxed) {
            0 => CircuitState::Closed,
            opened_at => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                if now - opened_at >= self.cooldown.as_secs() {
                    CircuitState::HalfOpen
                } else {
                    CircuitState::Open
                }
            }
        }
    }

    /// Check if a call is allowed.
    pub fn allow_request(&self) -> bool {
        match self.state() {
            CircuitState::Closed | CircuitState::HalfOpen => true,
            CircuitState::Open => false,
        }
    }

    /// Record a successful call.
    pub fn record_success(&self) {
        self.failure_count.store(0, Ordering::Relaxed);
        self.opened_at.store(0, Ordering::Relaxed);
    }

    /// Record a failed call.
    pub fn record_failure(&self) {
        let count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        if count >= self.failure_threshold {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            self.opened_at.store(now, Ordering::Relaxed);
        }
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(5, Duration::from_secs(30))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starts_closed() {
        let cb = CircuitBreaker::default();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());
    }

    #[test]
    fn test_opens_after_failures() {
        let cb = CircuitBreaker::new(3, Duration::from_secs(30));
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.allow_request());
    }

    #[test]
    fn test_resets_on_success() {
        let cb = CircuitBreaker::new(2, Duration::from_secs(30));
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }
}
