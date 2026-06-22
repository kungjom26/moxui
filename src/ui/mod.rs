//! Embedded web UI (Alpine.js SPA).
//!
//! All UI assets live under `ui/` at the workspace root and are baked
//! into the release binary via `rust-embed`. At runtime we serve:
//!
//! - `GET /`                  → `ui/index.html` (the SPA shell)
//! - `GET /static/*`          → `ui/static/**` (CSS, JS, vendor assets)
//! - `GET /static/app.js`     → `ui/static/app.js` (Alpine app logic)
//! - `GET /static/app.css`    → `ui/static/app.css` (hand-rolled CSS)
//! - `GET /static/vendor/*`   → `ui/static/vendor/*` (Alpine.js etc.)
//!
//! In dev (debug builds) we read from the workspace at startup so a
//! `cargo run` picks up live edits without rebuilding. In release
//! builds, the files are embedded into the binary so a single
//! `target/release/moxui` carries the full UI.
//!
//! The HTML is served with no-cache headers so an operator pushing a
//! new release sees the new UI immediately on next page load. Static
//! assets get `Cache-Control: max-age=300` (5 minutes) so a refresh
//! picks up UI changes promptly while still being cacheable.

use axum::{
    body::Body,
    extract::Path,
    http::{header, HeaderValue, Response, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "ui/"]
struct Asset;

/// Build a router that serves the embedded UI. Mount under `/` so:
/// - `GET /`            serves `index.html`
/// - `GET /static/*`    serves everything under `ui/static/`
/// - any other GET      serves `index.html` (SPA fallback for hash routing)
///
/// Generic over the state type `S` so this can be merged into a
/// `Router<AppState>` (the API) without losing the existing handlers
/// (none of which need state).
pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/", get(serve_index))
        .route("/static/*path", get(serve_static))
        .route("/locales/*path", get(serve_locales))
        .fallback(serve_index)
}

async fn serve_index() -> impl IntoResponse {
    serve_named("index.html", "text/html; charset=utf-8", false)
}

/// Serve a static asset. The `*path` wildcard in axum 0.7 captures
/// only the part after the parent route — e.g. for `/static/app.js`
/// the captured `path` is `app.js`. For `/static/vendor/alpine.min.js`
/// it's `vendor/alpine.min.js`. We re-prepend `static/` to look up
/// the asset.
async fn serve_static(Path(path): Path<String>) -> impl IntoResponse {
    // The path arrives URL-decoded; we accept it as-is. Re-prepend
    // `static/` to form the asset key in the embedded folder.
    let asset_name = format!("static/{path}");
    if Asset::get(&asset_name).is_none() {
        return not_found();
    }
    let mime = mime_guess::from_path(&path).first_or_octet_stream();
    serve_named(&asset_name, mime.as_ref(), true)
}

/// Serve locale files (JSON translation files).
///
/// e.g. `/locales/en.json` → `ui/locales/en.json`
async fn serve_locales(Path(path): Path<String>) -> impl IntoResponse {
    let asset_name = format!("locales/{path}");
    if Asset::get(&asset_name).is_none() {
        return not_found();
    }
    serve_named(&asset_name, "application/json; charset=utf-8", false)
}

fn serve_named(name: &str, content_type: &str, cacheable: bool) -> Response<Body> {
    match Asset::get(name) {
        Some(file) => {
            let body = Body::from(file.data.into_owned());
            let mut resp = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, content_type)
                .body(body)
                .expect("static response build");
            // Index should never be cached; static assets can be cached briefly.
            if cacheable {
                resp.headers_mut().insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=300"),
                );
            } else {
                resp.headers_mut().insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("no-cache, no-store, must-revalidate"),
                );
            }
            resp
        }
        None => not_found(),
    }
}

fn not_found() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::from("not found"))
        .expect("404 response build")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_serves_index_at_root() {
        let app = router();
        let resp = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp.headers().get(header::CONTENT_TYPE).unwrap();
        assert!(ct.to_str().unwrap().starts_with("text/html"));
        let body = to_bytes(resp.into_body(), 128 * 1024).await.unwrap();
        let s = std::str::from_utf8(&body).unwrap();
        assert!(s.contains("moxui"));
        assert!(s.contains("x-data=\"moxui()\"")); // Alpine root component
    }

    #[tokio::test]
    async fn test_serves_static_app_js() {
        let app = router();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/static/app.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), 192 * 1024).await.unwrap();
        let s = std::str::from_utf8(&body).unwrap();
        assert!(s.contains("function moxui()"));
    }

    #[tokio::test]
    async fn test_serves_static_app_css() {
        let app = router();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/static/app.css")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), 16 * 1024).await.unwrap();
        let s = std::str::from_utf8(&body).unwrap();
        assert!(s.contains("--bg:")); // CSS variable
    }

    #[tokio::test]
    async fn test_serves_static_vendor_alpine() {
        let app = router();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/static/vendor/alpine.min.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), 128 * 1024).await.unwrap();
        // Alpine.js is minified; just verify it's a non-trivial payload.
        assert!(
            body.len() > 10_000,
            "alpine.min.js should be >10KB, got {} bytes",
            body.len()
        );
    }

    #[tokio::test]
    async fn test_spa_fallback_serves_index_for_unknown_route() {
        // Hash-routing means the browser might hit /vms or /lxcs before
        // the SPA boots. We serve index.html for any unknown path so the
        // SPA can take over from there.
        let app = router();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/some/spa/route")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp.headers().get(header::CONTENT_TYPE).unwrap();
        assert!(ct.to_str().unwrap().starts_with("text/html"));
    }

    #[tokio::test]
    async fn test_index_has_no_cache_header() {
        let app = router();
        let resp = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let cache = resp.headers().get(header::CACHE_CONTROL).unwrap();
        let cache = cache.to_str().unwrap();
        assert!(cache.contains("no-cache"), "got: {cache}");
    }

    #[tokio::test]
    async fn test_static_has_short_cache_header() {
        let app = router();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/static/app.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let cache = resp.headers().get(header::CACHE_CONTROL).unwrap();
        let cache = cache.to_str().unwrap();
        assert!(cache.contains("max-age="), "got: {cache}");
    }
}
