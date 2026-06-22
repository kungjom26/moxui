//! OpenTelemetry tracing (OTLP exporter).
//!
//! Provides [`TracingConfig`] for configuration and [`init_tracing`] /
//! [`shutdown_tracing`] for lifecycle management.

use std::sync::Mutex;

use once_cell::sync::Lazy;
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::runtime::Tokio;
use opentelemetry_sdk::trace as sdktrace;
use opentelemetry_sdk::Resource;
use serde::Deserialize;

/// OpenTelemetry tracing configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct TracingConfig {
    /// Whether OTLP tracing is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// OTLP gRPC endpoint (e.g. `http://localhost:4317`).
    #[serde(default = "default_otlp_endpoint")]
    pub otlp_endpoint: String,
    /// Service name reported to the tracing backend.
    #[serde(default = "default_service_name")]
    pub service_name: String,
}

fn default_otlp_endpoint() -> String {
    "http://localhost:4317".to_string()
}

fn default_service_name() -> String {
    "moxui".to_string()
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            otlp_endpoint: "http://localhost:4317".to_string(),
            service_name: "moxui".to_string(),
        }
    }
}

/// Holds the tracer provider guard so it is not dropped before shutdown.
static TRACER_PROVIDER: Lazy<Mutex<Option<sdktrace::TracerProvider>>> =
    Lazy::new(|| Mutex::new(None));

/// Initialize the OpenTelemetry OTLP tracer.
///
/// Sets up an OTLP gRPC exporter that sends spans to `config.otlp_endpoint`.
/// The tracer is registered as the global tracer provider for
/// `tracing-opentelemetry` to pick up.
///
/// Returns `Ok(())` when successful (or when tracing is disabled). Call
/// [`shutdown_tracing`] before process exit to flush pending spans.
///
/// # Panics
///
/// Panics if called more than once (invariant: called once at startup).
pub fn init_tracing(config: &TracingConfig) -> anyhow::Result<()> {
    if !config.enabled {
        tracing::info!("OpenTelemetry tracing disabled");
        return Ok(());
    }

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&config.otlp_endpoint)
        .build()?;

    let provider = sdktrace::TracerProvider::builder()
        .with_batch_exporter(exporter, Tokio)
        .with_resource(Resource::new(vec![
            opentelemetry::KeyValue::new("service.name", config.service_name.clone()),
            opentelemetry::KeyValue::new("service.version", crate::VERSION.to_string()),
        ]))
        .build();

    let tracer = provider.tracer(config.service_name.clone());

    // Set as global tracer provider so `tracing-opentelemetry` can create
    // a layer that references it.
    let _ = opentelemetry::global::set_tracer_provider(provider.clone());

    // Store provider so it doesn't drop (which would shut down the exporter).
    let mut guard = TRACER_PROVIDER
        .lock()
        .expect("TRACER_PROVIDER mutex poisoned");
    *guard = Some(provider);

    tracing::info!(
        endpoint = %config.otlp_endpoint,
        service = %config.service_name,
        "OpenTelemetry tracing initialized"
    );

    // Ensure `tracer` is used (mark as used to avoid unused warning).
    let _ = tracer;

    Ok(())
}

/// Gracefully shut down the OTLP tracer, flushing any remaining spans.
///
/// Call this before process exit (e.g. in a signal handler or `Drop`).
pub fn shutdown_tracing() {
    let mut guard = TRACER_PROVIDER
        .lock()
        .expect("TRACER_PROVIDER mutex poisoned");
    if let Some(provider) = guard.take() {
        if let Err(e) = provider.shutdown() {
            tracing::warn!(error = %e, "OpenTelemetry tracer shutdown encountered error");
        }
        tracing::info!("OpenTelemetry tracer shut down");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_config_default() {
        let config = TracingConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.otlp_endpoint, "http://localhost:4317");
        assert_eq!(config.service_name, "moxui");
    }
}
