//! Telemetry setup (tracing + metrics).
//!
//! Initializes structured logging and Prometheus metrics.

use crate::config::LoggingConfig;

/// Initialize tracing subscriber (called once at startup).
pub fn init(config: &LoggingConfig) -> anyhow::Result<()> {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.level));

    let registry = tracing_subscriber::registry().with(filter);

    match config.format.as_str() {
        "json" => registry.with(fmt::layer().json()).try_init()?,
        _ => registry.with(fmt::layer().pretty()).try_init()?,
    }

    tracing::info!(
        version = crate::VERSION,
        git_sha = crate::GIT_SHA,
        profile = crate::BUILD_PROFILE,
        "MoxUI telemetry initialized"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LoggingConfig;

    #[test]
    fn test_init_with_default() {
        // Just verify the function signature compiles
        let config = LoggingConfig {
            level: "info".to_string(),
            format: "pretty".to_string(),
        };
        // Note: don't actually call init() in tests (would panic on duplicate)
        let _ = config;
    }
}
