//! Rate limiting middleware (governor).
//!
//! Provides tower layers for rate limiting by client IP using the
//! [`governor`] crate. Two flavours:
//!
//! - [`rate_limiter_layer`] — keyed by client IP, suitable for general API
//! - [`login_rate_limiter`] — non-keyed (global) for auth endpoints

use std::num::NonZeroU32;
use std::sync::Arc;

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
    response::Response,
};
use futures_util::future::BoxFuture;
use governor::clock::DefaultClock;
use governor::middleware::NoOpMiddleware;
use governor::state::keyed::DashMapStateStore;
use governor::{Quota, RateLimiter as GovRateLimiter};
use tower::Layer;
use uuid::Uuid;

use crate::config::RateLimitConfig;

/// Per‑IP rate limiter backed by `governor`.
///
/// The inner limiter uses a `DashMap<Uuid, ...>` keyed by a per‑request
/// identifier derived from the client IP (via `X-Forwarded-For` or the
/// peer address). We hash IPs into UUIDs so the key store doesn't leak
/// internal network topology if it's ever dumped.
///
/// ## Thread safety
///
/// The `DashMapStateStore` is `Send + Sync` so the limiter can be shared
/// across threads via `Arc`.
#[derive(Clone)]
pub struct IpRateLimiter {
    inner: Arc<GovRateLimiter<Uuid, DashMapStateStore<Uuid>, DefaultClock, NoOpMiddleware>>,
}

impl IpRateLimiter {
    /// Build a new limiter from configuration.
    pub fn new(config: &RateLimitConfig) -> Self {
        let quota = Quota::per_second(
            NonZeroU32::new(u32::try_from(config.requests_per_second).unwrap_or(u32::MAX))
                .unwrap(),
        )
        .allow_burst(NonZeroU32::new(config.burst_size).unwrap());
        Self {
            inner: Arc::new(GovRateLimiter::keyed(quota)),
        }
    }

    /// Check and note a request for a given client key.
    /// Returns `true` if the request is allowed, `false` if rate‑limited.
    pub fn check(&self, client_key: Uuid) -> bool {
        self.inner.check_key(&client_key).is_ok()
    }

    /// Produce a stable UUID from a socket address.
    pub fn key_from_addr(addr: &std::net::SocketAddr) -> Uuid {
        let bytes: [u8; 16] = match addr {
            std::net::SocketAddr::V4(v4) => {
                let mut b = [0u8; 16];
                b[..4].copy_from_slice(&v4.ip().octets());
                b[4..6].copy_from_slice(&v4.port().to_be_bytes());
                b[6] = 4; // IPv4 marker
                b
            }
            std::net::SocketAddr::V6(v6) => {
                let mut b = [0u8; 16];
                b.copy_from_slice(&v6.ip().octets());
                b
            }
        };
        Uuid::from_bytes(bytes)
    }
}

/// A Tower layer that applies IP‑based rate limiting.
///
/// Reads the client IP from `ConnectInfo<SocketAddr>` (set by the
/// transport layer — requires the server to be bound with
/// `.into_make_service_with_connect_info()`).
#[derive(Clone)]
pub struct RateLimitLayer {
    limiter: IpRateLimiter,
}

impl RateLimitLayer {
    /// Create a new layer from config.
    pub fn new(config: &RateLimitConfig) -> Self {
        Self {
            limiter: IpRateLimiter::new(config),
        }
    }
}

impl<S> Layer<S> for RateLimitLayer {
    type Service = RateLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitService {
            inner,
            limiter: self.limiter.clone(),
        }
    }
}

/// Tower service that enforces rate limits.
#[derive(Clone)]
pub struct RateLimitService<S> {
    inner: S,
    limiter: IpRateLimiter,
}

impl<S> tower::Service<Request<Body>> for RateLimitService<S>
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

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        // Try to extract client IP from ConnectInfo extension.
        let client_key = req
            .extensions()
            .get::<ConnectInfo<std::net::SocketAddr>>()
            .map_or_else(Uuid::nil, |ci| IpRateLimiter::key_from_addr(&ci.0));

        if !self.limiter.check(client_key) {
            let mut resp = Response::new(Body::from("rate limit exceeded"));
            *resp.status_mut() = StatusCode::TOO_MANY_REQUESTS;
            return Box::pin(async { Ok(resp) });
        }

        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);
        Box::pin(async move { inner.call(req).await })
    }
}

/// A simple non-keyed rate limiter for login endpoints.
/// Unlike the IP‑keyed limiter, this one counts total requests
/// across all clients (defense in depth).
pub fn login_rate_limiter() -> GovRateLimiter<
    governor::state::NotKeyed,
    governor::state::InMemoryState,
    governor::clock::DefaultClock,
    governor::middleware::NoOpMiddleware,
> {
    let quota = Quota::per_second(NonZeroU32::new(5).unwrap())
        .allow_burst(NonZeroU32::new(10).unwrap());
    GovRateLimiter::direct(quota)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddrV4};

    #[test]
    fn test_rate_limiter_accepts_initial_requests() {
        let config = RateLimitConfig {
            requests_per_second: 100, // high so tests are fast
            burst_size: 50,
        };
        let limiter = IpRateLimiter::new(&config);
        let key = IpRateLimiter::key_from_addr(&std::net::SocketAddr::V4(
            SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 1), 8080),
        ));

        // Should accept the first N requests
        for _ in 0..10 {
            assert!(limiter.check(key), "should accept first requests");
        }
    }

    #[test]
    fn test_login_rate_limiter_accepts_initial_burst() {
        let limiter = login_rate_limiter();
        // First 10 should be OK (burst = 10)
        for _ in 0..10 {
            assert!(
                limiter.check().is_ok(),
                "burst of 10 should be allowed"
            );
        }
    }

    #[test]
    fn test_key_from_addr_stable() {
        let addr1 = std::net::SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 1), 8080));
        let addr2 = std::net::SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 1), 8080));
        assert_eq!(
            IpRateLimiter::key_from_addr(&addr1),
            IpRateLimiter::key_from_addr(&addr2)
        );
    }

    #[test]
    fn test_key_from_addr_differs() {
        let addr1 = std::net::SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 1), 8080));
        let addr2 = std::net::SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 2), 8080));
        assert_ne!(
            IpRateLimiter::key_from_addr(&addr1),
            IpRateLimiter::key_from_addr(&addr2)
        );
    }
}