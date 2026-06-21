//! Proxmox API client (ticket auth + connection pool + circuit breaker).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;

use crate::config::ClusterConfig;
use crate::error::{AppError, AppResult};
use crate::proxmox::auth::{Ticket, TicketResponse};
use crate::proxmox::circuit_breaker::CircuitBreaker;
use crate::proxmox::retry::RetryPolicy;

use reqwest::Client;

/// Proxmox API client for one cluster.
pub struct ProxmoxClient {
    /// Cluster configuration.
    config: ClusterConfig,
    /// HTTP client (connection pool).
    http: Client,
    /// Current ticket (auto-refreshed).
    ticket: Arc<RwLock<Option<Ticket>>>,
    /// Circuit breaker.
    circuit_breaker: CircuitBreaker,
    /// Retry policy (consumed by `request()` helper in Day 2).
    #[allow(dead_code)]
    retry_policy: RetryPolicy,
}

impl ProxmoxClient {
    /// Create a new Proxmox client for the given cluster.
    #[allow(clippy::unused_async)] // async signature kept for API stability — will become async when login pre-warm is added
    pub async fn new(config: ClusterConfig) -> AppResult<Self> {
        let mut builder = Client::builder()
            .danger_accept_invalid_certs(config.insecure_skip_verify)
            .timeout(Duration::from_secs(30));

        // If CA cert is provided, use it for verification
        if let Some(ca_pem) = &config.ca_cert_pem {
            let cert = reqwest::Certificate::from_pem(ca_pem.as_bytes())
                .map_err(|e| AppError::Config(format!("Invalid CA cert: {e}")))?;
            builder = builder.add_root_certificate(cert);
        }

        let http = builder
            .build()
            .map_err(|e| AppError::Config(format!("Failed to build HTTP client: {e}")))?;

        Ok(Self {
            config,
            http,
            ticket: Arc::new(RwLock::new(None)),
            circuit_breaker: CircuitBreaker::default(),
            retry_policy: RetryPolicy::default(),
        })
    }

    /// Get the cluster URL.
    pub fn url(&self) -> &str {
        &self.config.url
    }

    /// Get the cluster name.
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Check if a request is allowed (circuit breaker state).
    pub fn allow_request(&self) -> bool {
        self.circuit_breaker.allow_request()
    }

    /// Login to Proxmox and get a ticket.
    pub async fn login(&self) -> AppResult<()> {
        let url = format!("{}/api2/json/access/ticket", self.config.url);
        let resp = self
            .http
            .post(&url)
            .json(&serde_json::json!({
                "username": &self.config.username,
                "password": &self.config.password,
            }))
            .send()
            .await?;

        if !resp.status().is_success() {
            self.circuit_breaker.record_failure();
            return Err(AppError::Proxmox(format!(
                "Login failed with status {}",
                resp.status()
            )));
        }

        let body: TicketResponse = resp.json().await?;
        let mut guard = self.ticket.write().await;
        *guard = Some(body.data);
        self.circuit_breaker.record_success();
        Ok(())
    }

    /// Ensure we have a valid ticket (refresh if <5min to expiry).
    pub async fn ensure_ticket(&self) -> AppResult<()> {
        let should_refresh = {
            let guard = self.ticket.read().await;
            match guard.as_ref() {
                Some(t) => {
                    let now = chrono::Utc::now().timestamp();
                    t.expires_at - now < 300 // refresh 5 min before expiry
                }
                None => true,
            }
        };

        if should_refresh {
            self.login().await?;
        }
        Ok(())
    }

    /// Get current ticket (forces login if missing).
    pub async fn current_ticket(&self) -> AppResult<Ticket> {
        self.ensure_ticket().await?;
        let guard = self.ticket.read().await;
        guard
            .clone()
            .ok_or_else(|| AppError::Internal("ticket missing after ensure".into()))
    }

    /// Get the underlying HTTP client (for requests).
    pub fn http(&self) -> &Client {
        &self.http
    }

    /// Record success (reset circuit breaker).
    pub fn record_success(&self) {
        self.circuit_breaker.record_success();
    }

    /// Record failure (may open circuit breaker).
    pub fn record_failure(&self) {
        self.circuit_breaker.record_failure();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cluster_config() -> ClusterConfig {
        ClusterConfig {
            name: "test".to_string(),
            url: "https://test.local:8006".to_string(),
            username: "root@pam".to_string(),
            password: "test".to_string(),
            realm: "pam".to_string(),
            insecure_skip_verify: true,
            ca_cert_pem: None,
        }
    }

    #[tokio::test]
    async fn test_client_creation() {
        let config = test_cluster_config();
        let client = ProxmoxClient::new(config).await;
        assert!(client.is_ok());
    }

    #[test]
    fn test_url_and_name() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let config = test_cluster_config();
        let client = rt.block_on(ProxmoxClient::new(config)).unwrap();
        assert_eq!(client.name(), "test");
        assert_eq!(client.url(), "https://test.local:8006");
    }
}
