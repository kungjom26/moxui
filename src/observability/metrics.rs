//! Prometheus metrics (RED + USE).
//!
//! Defines RED metrics (Rate, Errors, Duration) for HTTP handlers and USE
//! metrics (Utilization, Saturation, Errors) for Proxmox clients and cache.
//!
//! RED metrics are recorded automatically via [`MetricsLayer`] (a Tower
//! middleware). USE metrics are recorded explicitly via helper methods
//! on [`MetricsService`].

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Instant;

use axum::extract::Request;
use axum::response::Response;
use prometheus::{
    register_counter_vec_with_registry, register_counter_with_registry,
    register_gauge_vec_with_registry, register_histogram_vec_with_registry, Counter, CounterVec,
    GaugeVec, HistogramVec, Registry,
};
use tower::{Layer, Service};

/// Wraps a Prometheus registry and all metric handles for the application.
///
/// Clone is cheap (inner `Arc`).
#[derive(Clone)]
pub struct MetricsService {
    inner: Arc<MetricsInner>,
}

struct MetricsInner {
    registry: Registry,

    // ── RED metrics (Rate, Errors, Duration) ──────────────────────
    /// Total HTTP requests, labelled by method, path, and response status.
    http_requests_total: CounterVec,
    /// Histogram of request durations (seconds), labelled by method and path.
    http_request_duration_seconds: HistogramVec,
    /// Current number of in-flight HTTP requests.
    http_requests_in_flight: GaugeVec,

    // ── USE metrics (Utilization, Saturation, Errors) ─────────────
    /// 1 = healthy, 0 = unhealthy, one time-series per cluster.
    proxmox_clients_healthy: GaugeVec,
    /// Total cache hits.
    proxmox_cache_hits_total: Counter,
    /// Total cache misses.
    proxmox_cache_misses_total: Counter,
    /// Total audit-log entries written.
    audit_log_entries_total: Counter,
}

impl MetricsService {
    /// Create a new `MetricsService` with a fresh Prometheus registry and
    /// all metric variables registered.
    #[allow(clippy::too_many_lines)] // metric registration is naturally verbose
    pub fn new() -> Self {
        let registry = Registry::new();

        let http_requests_total = register_counter_vec_with_registry!(
            "http_requests_total",
            "Total HTTP requests",
            &["method", "path", "status"],
            registry
        )
        .expect("http_requests_total");

        let http_request_duration_seconds = register_histogram_vec_with_registry!(
            "http_request_duration_seconds",
            "HTTP request duration in seconds",
            &["method", "path"],
            // Default buckets cover typical web latencies
            vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0],
            registry,
        )
        .expect("http_request_duration_seconds");

        let http_requests_in_flight = register_gauge_vec_with_registry!(
            "http_requests_in_flight",
            "Current number of in-flight HTTP requests",
            &["method"],
            registry,
        )
        .expect("http_requests_in_flight");

        let proxmox_clients_healthy = register_gauge_vec_with_registry!(
            "proxmox_clients_healthy",
            "Proxmox cluster health (1 = healthy, 0 = unhealthy)",
            &["cluster"],
            registry,
        )
        .expect("proxmox_clients_healthy");

        let proxmox_cache_hits_total = register_counter_with_registry!(
            "proxmox_cache_hits_total",
            "Total Proxmox cache hits",
            registry,
        )
        .expect("proxmox_cache_hits_total");

        let proxmox_cache_misses_total = register_counter_with_registry!(
            "proxmox_cache_misses_total",
            "Total Proxmox cache misses",
            registry,
        )
        .expect("proxmox_cache_misses_total");

        let audit_log_entries_total = register_counter_with_registry!(
            "audit_log_entries_total",
            "Total audit log entries written",
            registry,
        )
        .expect("audit_log_entries_total");

        Self {
            inner: Arc::new(MetricsInner {
                registry,
                http_requests_total,
                http_request_duration_seconds,
                http_requests_in_flight,
                proxmox_clients_healthy,
                proxmox_cache_hits_total,
                proxmox_cache_misses_total,
                audit_log_entries_total,
            }),
        }
    }

    /// Return a reference to the Prometheus registry (for the `/metrics`
    /// endpoint to encode).
    pub fn registry(&self) -> &Registry {
        &self.inner.registry
    }

    /// Record a Proxmox cache hit or miss.
    pub fn record_proxmox_cache(&self, hit: bool) {
        if hit {
            self.inner.proxmox_cache_hits_total.inc();
        } else {
            self.inner.proxmox_cache_misses_total.inc();
        }
    }

    /// Record one audit-log entry being written.
    pub fn record_audit_log(&self) {
        self.inner.audit_log_entries_total.inc();
    }

    /// Set the health gauge for a cluster (1 = healthy, 0 = unhealthy).
    pub fn set_cluster_health(&self, cluster: &str, healthy: bool) {
        let v = if healthy { 1.0 } else { 0.0 };
        self.inner
            .proxmox_clients_healthy
            .with_label_values(&[cluster])
            .set(v);
    }

    /// Record the start of an HTTP request (increment in-flight gauge).
    fn start_request(&self, method: &str) {
        self.inner
            .http_requests_in_flight
            .with_label_values(&[method])
            .inc();
    }

    /// Record the end of an HTTP request (duration, status, decrement in-flight).
    fn end_request(&self, method: &str, path: &str, status: u16, start: Instant) {
        let duration = start.elapsed().as_secs_f64();

        self.inner
            .http_requests_total
            .with_label_values(&[method, path, &status.to_string()])
            .inc();

        self.inner
            .http_request_duration_seconds
            .with_label_values(&[method, path])
            .observe(duration);

        self.inner
            .http_requests_in_flight
            .with_label_values(&[method])
            .dec();
    }
}

impl Default for MetricsService {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tower Layer ───────────────────────────────────────────────────────────

/// A Tower [`Layer`] that wraps every request with RED metric recording.
///
/// Add this layer to the public / outer router so all endpoints (including
/// health checks) are instrumented.
#[derive(Clone)]
pub struct MetricsLayer {
    metrics: MetricsService,
}

impl MetricsLayer {
    /// Create a new layer from a shared [`MetricsService`].
    pub fn new(metrics: MetricsService) -> Self {
        Self { metrics }
    }
}

impl<S> Layer<S> for MetricsLayer {
    type Service = MetricsMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        MetricsMiddleware {
            inner,
            metrics: self.metrics.clone(),
        }
    }
}

/// Tower [`Service`] that records RED metrics around each request.
#[derive(Clone)]
pub struct MetricsMiddleware<S> {
    inner: S,
    metrics: MetricsService,
}

impl<S, ReqBody, ResBody: 'static> Service<Request<ReqBody>> for MetricsMiddleware<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
    S::Future: Send + 'static,
    S::Error: std::error::Error + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<S::Response, S::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let method = req.method().to_string();
        let path = req.uri().path().to_string();
        let metrics = self.metrics.clone();
        let start = Instant::now();

        metrics.start_request(&method);

        let inner = self.inner.call(req);

        Box::pin(async move {
            match inner.await {
                Ok(response) => {
                    let status = response.status().as_u16();
                    metrics.end_request(&method, &path, status, start);
                    Ok(response)
                }
                Err(e) => {
                    tracing::warn!(error = %e, "metrics middleware: request error");
                    metrics.end_request(&method, &path, 500, start);
                    Err(e)
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_service_creation() {
        let ms = MetricsService::new();
        // Default state: zero-valued metrics
        let families = ms.registry().gather();
        assert!(!families.is_empty(), "should have registered metrics");
    }

    #[test]
    fn test_cache_hit_miss() {
        let ms = MetricsService::new();
        ms.record_proxmox_cache(true);
        ms.record_proxmox_cache(false);
        ms.record_proxmox_cache(true);
        // Can't easily assert counter values from outside the registry,
        // but we verify no panics.
    }

    #[test]
    fn test_cluster_health() {
        let ms = MetricsService::new();
        ms.set_cluster_health("test-cluster", true);
        ms.set_cluster_health("test-cluster", false);
    }

    #[test]
    fn test_audit_log() {
        let ms = MetricsService::new();
        ms.record_audit_log();
    }
}
