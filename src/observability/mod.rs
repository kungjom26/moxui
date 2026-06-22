//! Observability (metrics + tracing exporters).
//!
//! - `metrics`: Prometheus RED + USE metric definitions, a [`MetricsLayer`]
//!   Tower middleware, and the [`MetricsService`] handle.
//! - `tracing`: OpenTelemetry OTLP exporter setup with [`TracingConfig`].

pub mod metrics;
pub mod tracing;
