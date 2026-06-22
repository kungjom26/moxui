//! Security middleware (rate limit, CORS, API keys, security headers).

pub mod api_key;
pub mod rate_limiter;

pub use api_key::{ApiKeyAuthenticated, ApiKeyConfig, ApiKeyLayer};
pub use rate_limiter::{login_rate_limiter, IpRateLimiter, RateLimitLayer, RateLimitService};

/// Build a CORS layer from configuration.
///
/// When `allowed_origins` is empty, permits all origins (*) — useful
/// for development. In production, restrict to known origins.
pub fn cors_layer(config: &crate::config::CorsConfig) -> tower_http::cors::CorsLayer {
    use axum::http::header::HeaderValue;
    if config.allowed_origins.is_empty() {
        tower_http::cors::CorsLayer::permissive()
    } else {
        let origins: Vec<HeaderValue> = config
            .allowed_origins
            .iter()
            .filter_map(|o| o.parse::<HeaderValue>().ok())
            .collect();
        tower_http::cors::CorsLayer::new()
            .allow_origin(origins)
            .allow_methods([
                axum::http::Method::GET,
                axum::http::Method::POST,
                axum::http::Method::PUT,
                axum::http::Method::DELETE,
                axum::http::Method::PATCH,
                axum::http::Method::OPTIONS,
            ])
            .allow_headers([
                axum::http::header::CONTENT_TYPE,
                axum::http::header::AUTHORIZATION,
                axum::http::header::HeaderName::from_static("x-api-key"),
            ])
            .max_age(std::time::Duration::from_secs(config.max_age_secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cors_permissive_when_empty() {
        let cfg = crate::config::CorsConfig::default();
        let _layer = cors_layer(&cfg);
        // layer builds without panic
    }

    #[test]
    fn test_cors_restrictive_with_origins() {
        let cfg = crate::config::CorsConfig {
            allowed_origins: vec!["https://moxui.example.com".into()],
            max_age_secs: 3600,
        };
        let _layer = cors_layer(&cfg);
    }
}
