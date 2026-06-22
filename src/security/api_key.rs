//! API key authentication middleware.
//!
//! Provides an alternative auth method to JWT: clients send
//! `X-API-Key: <shared-secret>` in request headers. When API key
//! auth is enabled in config, the middleware accepts requests with
//! a valid API key in addition to (or instead of) Bearer JWT tokens.
//!
//! API keys are flat shared secrets — they don't identify individual
//! users. They're best suited for:
//! - Automation / CI pipelines
//! - Monitoring integrations
//! - Read‑only API access (when combined with a viewer role)

use axum::{
    body::Body,
    http::Request,
    response::Response,
};
use futures_util::future::BoxFuture;
use tower::Layer;

/// Key used to store the `X-API-Key` header value.
const API_KEY_HEADER: &str = "x-api-key";

/// Configuration for API key auth.
#[derive(Clone, Debug)]
pub struct ApiKeyConfig {
    /// Whether API key auth is enabled.
    pub enabled: bool,
    /// The expected API key value.
    pub key: Option<String>,
}

impl From<crate::config::ApiKeyConfig> for ApiKeyConfig {
    fn from(cfg: crate::config::ApiKeyConfig) -> Self {
        Self {
            enabled: cfg.enabled,
            key: cfg.key,
        }
    }
}

/// A Tower layer that checks `X-API-Key` headers against a configured
/// shared secret.
///
/// The layer does not reject requests — it only **marks** the request
/// as authenticated via an extension. Downstream middleware or handlers
/// check for the extension. This allows API key auth to coexist with
/// Bearer JWT auth (the JWT middleware can also accept the request).
///
/// When the API key matches, `ApiKeyAuthenticated` is inserted as a
/// request extension.
#[derive(Clone)]
pub struct ApiKeyLayer {
    config: ApiKeyConfig,
}

impl ApiKeyLayer {
    /// Create a new layer from config.
    pub fn new(config: impl Into<ApiKeyConfig>) -> Self {
        Self {
            config: config.into(),
        }
    }
}

impl<S> Layer<S> for ApiKeyLayer {
    type Service = ApiKeyService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ApiKeyService {
            inner,
            config: self.config.clone(),
        }
    }
}

/// Extension marker inserted into the request when API key auth succeeds.
/// Downstream middleware or handlers can check for this to know that
/// the request was authenticated via API key.
#[derive(Debug, Clone)]
pub struct ApiKeyAuthenticated;

/// Tower service that performs API key header check.
#[derive(Clone)]
pub struct ApiKeyService<S> {
    inner: S,
    config: ApiKeyConfig,
}

impl<S> tower::Service<Request<Body>> for ApiKeyService<S>
where
    S: tower::Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        // Check API key — insert extension if valid
        if self.config.enabled {
            if let Some(cfg_key) = &self.config.key {
                if req.headers().get(API_KEY_HEADER).is_some_and(|v| v.as_bytes() == cfg_key.as_bytes()) {
                    req.extensions_mut().insert(ApiKeyAuthenticated);
                }
            }
        }

        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);
        Box::pin(async move { inner.call(req).await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_api_key_missing_when_disabled() {
        let config = ApiKeyConfig {
            enabled: false,
            key: Some("s3cr3t".into()),
        };
        let layer = ApiKeyLayer::new(config);
        let svc = layer.layer(tower::service_fn(|req: Request<Body>| async move {
            let has_ext = req.extensions().get::<ApiKeyAuthenticated>().is_some();
            Ok::<_, std::convert::Infallible>(Response::new(Body::from(if has_ext {
                "authed"
            } else {
                "noauth"
            })))
        }));

        let resp = svc
            .oneshot(
                Request::builder()
                    .header(API_KEY_HEADER, "s3cr3t")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = to_bytes(resp.into_body(), 32).await.unwrap();
        let s = std::str::from_utf8(&body).unwrap();
        assert_eq!(s, "noauth", "disabled = no auth");
    }

    #[tokio::test]
    async fn test_api_key_correct() {
        let config = ApiKeyConfig {
            enabled: true,
            key: Some("s3cr3t".into()),
        };
        let layer = ApiKeyLayer::new(config);
        let svc = layer.layer(tower::service_fn(|req: Request<Body>| async move {
            let has_ext = req.extensions().get::<ApiKeyAuthenticated>().is_some();
            Ok::<_, std::convert::Infallible>(Response::new(Body::from(if has_ext {
                "authed"
            } else {
                "noauth"
            })))
        }));

        let resp = svc
            .oneshot(
                Request::builder()
                    .header(API_KEY_HEADER, "s3cr3t")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = to_bytes(resp.into_body(), 32).await.unwrap();
        let s = std::str::from_utf8(&body).unwrap();
        assert_eq!(s, "authed", "correct key = authenticated");
    }

    #[tokio::test]
    async fn test_api_key_wrong() {
        let config = ApiKeyConfig {
            enabled: true,
            key: Some("s3cr3t".into()),
        };
        let layer = ApiKeyLayer::new(config);
        let svc = layer.layer(tower::service_fn(|req: Request<Body>| async move {
            let has_ext = req.extensions().get::<ApiKeyAuthenticated>().is_some();
            Ok::<_, std::convert::Infallible>(Response::new(Body::from(if has_ext {
                "authed"
            } else {
                "noauth"
            })))
        }));

        let resp = svc
            .oneshot(
                Request::builder()
                    .header(API_KEY_HEADER, "wrong")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = to_bytes(resp.into_body(), 32).await.unwrap();
        let s = std::str::from_utf8(&body).unwrap();
        assert_eq!(s, "noauth", "wrong key = no auth");
    }

    #[tokio::test]
    async fn test_api_key_not_sent() {
        let config = ApiKeyConfig {
            enabled: true,
            key: Some("s3cr3t".into()),
        };
        let layer = ApiKeyLayer::new(config);
        let svc = layer.layer(tower::service_fn(|req: Request<Body>| async move {
            let has_ext = req.extensions().get::<ApiKeyAuthenticated>().is_some();
            Ok::<_, std::convert::Infallible>(Response::new(Body::from(if has_ext {
                "authed"
            } else {
                "noauth"
            })))
        }));

        let resp = svc
            .oneshot(
                Request::builder()
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = to_bytes(resp.into_body(), 32).await.unwrap();
        let s = std::str::from_utf8(&body).unwrap();
        assert_eq!(s, "noauth", "no header = no auth");
    }
}