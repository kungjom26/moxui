//! Auth middleware — extract & validate a `Bearer` JWT from the
//! `Authorization` header, decode it with [`JwtService`], and store the
//! [`Claims`] in the request extensions so handlers can read them via
//! the [`AuthContext`] extractor.
//!
//! Two layers:
//!
//! - [`require_auth`] — middleware function. Returns 401 if the header is
//!   missing/malformed/expired. Use this to protect entire route scopes
//!   (e.g. all `/api/v1/vms/.../:action` writes).
//!
//! - [`AuthContext`] — `axum::extract::FromRequestParts` extractor. Reads
//!   the `Claims` that [`require_auth`] inserted. Use this in handlers
//!   that need to know *who* is calling. Pairs with [`require_role`] to
//!   enforce RBAC inside a handler.

use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

use super::{jwt::Claims, user::Role};

/// Bearer token prefix in the `Authorization` header.
const BEARER_PREFIX: &str = "Bearer ";

/// Axum middleware — require a valid `Authorization: Bearer *** header.
///
/// On success, the decoded [`Claims`] are stored in the request extensions
/// so [`AuthContext`] (or any other extractor) can read them.
///
/// # Errors
///
/// - `401 Unauthorized` if the header is missing, malformed, or the token
///   fails to decode (expired / wrong signature / wrong audience).
pub async fn require_auth(
    axum::extract::State(state): axum::extract::State<crate::state::AppState>,
    mut request: axum::extract::Request,
    next: Next,
) -> Response {
    let token = match extract_bearer(&request) {
        Ok(t) => t,
        Err(resp) => return resp,
    };
    match state.jwt.decode(&token) {
        Ok(claims) => {
            request.extensions_mut().insert(claims);
            next.run(request).await
        }
        Err(e) => {
            tracing::debug!(error = %e, "rejected invalid JWT");
            (StatusCode::UNAUTHORIZED, "Invalid or expired token").into_response()
        }
    }
}

/// Axum middleware — require that the calling user has access to the
/// cluster identified by the `:cluster` path parameter.
///
/// Must be applied AFTER [`require_auth`] so the [`Claims`] are available
/// in the request extensions. Extracts the cluster name from the 4th
/// path segment of `/api/v1/{resource}/{cluster}/...` and checks it
/// against the user's `allowed_clusters` via [`UserStore::user_can_access_cluster`].
///
/// Users with unrestricted access (admin, or no allowed_clusters configured)
/// are always allowed.
///
/// # Errors
///
/// - `403 Forbidden` if the user is not allowed to access the cluster.
/// - `400 Bad Request` if no cluster segment is found in the path.
pub async fn require_cluster_access(
    axum::extract::State(state): axum::extract::State<crate::state::AppState>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    // Extract claims set by require_auth.
    let claims = match request.extensions().get::<Claims>() {
        Some(c) => c.clone(),
        None => {
            tracing::warn!("require_cluster_access called without require_auth — misconfigured route");
            return unauthorized("auth context missing — require_auth must precede require_cluster_access");
        }
    };

    // Extract cluster name from the URL path.
    // Format: /api/v1/{resource}/{cluster}/...
    // The cluster is always the 4th path segment (index 3).
    let cluster = match extract_cluster_from_path(request.uri().path()) {
        Some(c) => c,
        None => {
            tracing::warn!(
                path = %request.uri().path(),
                "require_cluster_access: no cluster segment found in path"
            );
            return (StatusCode::BAD_REQUEST, "No cluster specified in request path").into_response();
        }
    };

    // Check access.
    if state.users.user_can_access_cluster(&claims.username, &cluster) {
        next.run(request).await
    } else {
        tracing::warn!(
            user = %claims.username,
            cluster = %cluster,
            "cluster access denied"
        );
        forbidden(&format!(
            "Access denied to cluster '{cluster}'"
        ))
    }
}

/// Extractor that reads the [`Claims`] previously stored by [`require_auth`].
///
/// Use this in handlers that need to know the calling user. If the
/// middleware was not applied to the route, [`AuthContext::from_request_parts`]
/// returns 500 (programmer error — should never happen in normal use).
pub struct AuthContext {
    /// Decoded JWT claims for the calling user.
    pub claims: Claims,
}

impl AuthContext {
    /// Read the role from the claims as the typed [`Role`] enum.
    ///
    /// Returns `Err(role_string)` if the claim contains an unrecognised
    /// role (e.g. token was issued by an old moxui build with a role
    /// that's been renamed).
    pub fn role(&self) -> Result<Role, String> {
        self.claims.role.parse()
    }
}

impl<S> FromRequestParts<S> for AuthContext
where
    S: Send + Sync,
{
    type Rejection = Response;

    fn from_request_parts<'life0, 'life1, 'async_trait>(
        parts: &'life0 mut Parts,
        _state: &'life1 S,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self, Self::Rejection>> + Send + 'async_trait>,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        // Claims were inserted by `require_auth` middleware. If we got
        // here without that middleware, the route is misconfigured.
        let claims = parts.extensions.get::<Claims>().cloned().ok_or_else(|| {
            unauthorized("auth context missing — require_auth middleware not applied")
        });
        Box::pin(async move { claims.map(|c| Self { claims: c }) })
    }
}

/// Check that the calling user satisfies the required role. Returns
/// 403 Forbidden if not.
///
/// Use inside a handler that has already extracted an [`AuthContext`]:
///
/// ```ignore
/// async fn handler(auth: AuthContext) -> AppResult<impl IntoResponse> {
///     require_role(&auth, Role::Operator)?;
///     // ... do the thing
/// }
/// ```
#[allow(clippy::result_large_err)] // Response is the natural error type here
pub fn require_role(auth: &AuthContext, required: Role) -> Result<(), Response> {
    match auth.role() {
        Ok(role) if role.can(required) => Ok(()),
        Ok(role) => {
            tracing::warn!(
                user = %auth.claims.username,
                role = %role,
                required = %required,
                "RBAC denial"
            );
            Err(forbidden("insufficient role"))
        }
        Err(bad_role) => Err(forbidden(&format!("unknown role in token: {bad_role}"))),
    }
}

/// Pull the `Authorization: Bearer *** header from a request. Returns 401
/// if the header is absent or doesn't start with `Bearer `.
fn extract_bearer(request: &axum::extract::Request) -> Result<String, Response> {
    let header = request
        .headers()
        .get(AUTHORIZATION)
        .ok_or_else(|| unauthorized("missing Authorization header"))?
        .to_str()
        .map_err(|_| unauthorized("malformed Authorization header"))?;
    let token = header
        .strip_prefix(BEARER_PREFIX)
        .ok_or_else(|| unauthorized("Authorization must be Bearer scheme"))?;
    if token.is_empty() {
        return Err(unauthorized("empty Bearer token"));
    }
    Ok(token.to_string())
}

fn unauthorized(msg: &str) -> Response {
    (StatusCode::UNAUTHORIZED, msg.to_string()).into_response()
}

fn forbidden(msg: &str) -> Response {
    (StatusCode::FORBIDDEN, msg.to_string()).into_response()
}

/// Extract the cluster name from a URL path like `/api/v1/vms/{cluster}/...`.
///
/// Returns the 4th path segment (index 3) when the path starts with
/// `/api/v1/`, which is where `:cluster` params sit in all current routes.
/// Returns `None` if the path is too short or doesn't match the expected
/// prefix — including for routes that don't carry a cluster param at all
/// (e.g. `/api/v1/dashboard`), which means the middleware can safely be
/// applied to any protected route without false positives.
fn extract_cluster_from_path(path: &str) -> Option<String> {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    // /api/v1/{resource}/{cluster}/...
    if segments.len() >= 4 && segments[0] == "api" && segments[1] == "v1" {
        Some(segments[3].to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::JwtService;

    #[test]
    fn bearer_prefix_constant() {
        assert_eq!(BEARER_PREFIX, "Bearer ");
    }

    #[test]
    fn require_role_allows_admin_to_viewer() {
        let svc = test_service();
        let claims = make_claims("admin");
        let token = svc.encode(&claims).unwrap();
        let claims = svc.decode(&token).unwrap();
        let auth = AuthContext { claims };
        assert!(require_role(&auth, Role::Viewer).is_ok());
        assert!(require_role(&auth, Role::Operator).is_ok());
        assert!(require_role(&auth, Role::Admin).is_ok());
    }

    #[test]
    fn require_role_blocks_viewer_from_admin() {
        let svc = test_service();
        let claims = make_claims("viewer");
        let token = svc.encode(&claims).unwrap();
        let claims = svc.decode(&token).unwrap();
        let auth = AuthContext { claims };
        assert!(require_role(&auth, Role::Viewer).is_ok());
        let err = require_role(&auth, Role::Admin).unwrap_err();
        assert_eq!(err.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn require_role_blocks_viewer_from_operator() {
        let svc = test_service();
        let claims = make_claims("viewer");
        let token = svc.encode(&claims).unwrap();
        let claims = svc.decode(&token).unwrap();
        let auth = AuthContext { claims };
        let err = require_role(&auth, Role::Operator).unwrap_err();
        assert_eq!(err.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn auth_context_role_parses_known() {
        let claims = make_claims("operator");
        let auth = AuthContext { claims };
        assert_eq!(auth.role().unwrap(), Role::Operator);
    }

    #[test]
    fn auth_context_role_rejects_unknown() {
        let claims = make_claims("wizard");
        let auth = AuthContext { claims };
        assert!(auth.role().is_err());
    }

    fn make_claims(role: &str) -> Claims {
        let now = chrono::Utc::now().timestamp();
        Claims {
            sub: "u-test".to_string(),
            username: "alice".to_string(),
            role: role.to_string(),
            iat: now,
            exp: now + 60,
        }
    }

    fn test_service() -> JwtService {
        const PRIV_PEM: &str = include_str!("../../tests/fixtures/test_jwt_priv.pem");
        const PUB_PEM: &str = include_str!("../../tests/fixtures/test_jwt_pub.pem");
        JwtService::new(
            PRIV_PEM.as_bytes(),
            PUB_PEM.as_bytes(),
            "moxui-test",
            "moxui-test",
        )
        .expect("test keypair")
    }
}
