//! TLS configuration + HTTPS server bootstrap.
//!
//! When the operator configures `server.tls`, we listen with
//! `axum-server` + rustls and refuse plaintext HTTP. When TLS is not
//! configured, we fall back to plaintext HTTP with a startup warning
//! (development mode).
//!
//! Cert format: PEM-encoded certificate chain + private key. The cert
//! should be a full chain (leaf + intermediates) so browsers/clients
//! don't have to chase issuers out-of-band.

use std::net::SocketAddr;
use std::path::Path;

use anyhow::{Context, Result};
use axum_server::tls_rustls::RustlsConfig;

use crate::config::TlsConfig;

/// Load an axum-server [`RustlsConfig`] from the cert+key paths in
/// `TlsConfig`. Convenience wrapper around `RustlsConfig::from_pem_file`.
pub async fn load_rustls_config(tls: &TlsConfig) -> Result<RustlsConfig> {
    RustlsConfig::from_pem_file(Path::new(&tls.cert_pem_path), Path::new(&tls.key_pem_path))
        .await
        .with_context(|| {
            format!(
                "loading TLS cert+key from {} + {}",
                tls.cert_pem_path, tls.key_pem_path
            )
        })
}

/// Bind an axum [`axum::serve::Serve`] (or `axum-server::tls_rustls::TlsServer`)
/// to `addr` and serve the given `app` over the requested transport.
///
/// Returns when the server is shut down (e.g. via SIGINT / `tokio::signal`).
pub async fn serve(addr: SocketAddr, app: axum::Router, tls: Option<&TlsConfig>) -> Result<()> {
    if let Some(tls_cfg) = tls {
        tracing::info!(%addr, "MoxUI listening (HTTPS)");
        let rustls_cfg = load_rustls_config(tls_cfg).await?;
        let server = axum_server::bind_rustls(addr, rustls_cfg);
        server
            .serve(app.into_make_service())
            .await
            .context("axum_server::tls_rustls::serve")?;
    } else {
        tracing::info!(%addr, "MoxUI listening (plaintext HTTP — dev mode)");
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .with_context(|| format!("binding plaintext listener to {addr}"))?;
        axum::serve(listener, app.into_make_service())
            .await
            .context("axum::serve")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TlsConfig;
    use rustls::ClientConfig;
    use tokio_rustls::TlsConnector;

    /// Install rustls crypto provider once per test binary. Required by
    /// rustls 0.23+ — every TLS-touching test panics without it.
    fn ensure_crypto_provider() {
        use std::sync::Once;
        static ONCE: Once = Once::new();
        ONCE.call_once(crate::install_crypto_provider);
    }

    fn fixture_paths() -> TlsConfig {
        TlsConfig {
            cert_pem_path: "tests/fixtures/test_tls_cert.pem".to_string(),
            key_pem_path: "tests/fixtures/test_tls_key.pem".to_string(),
        }
    }

    #[tokio::test]
    async fn load_rustls_config_succeeds_with_valid_pem() {
        ensure_crypto_provider();
        let cfg = fixture_paths();
        let rustls_cfg = load_rustls_config(&cfg)
            .await
            .expect("valid PEM should load");
        // We don't care about internals — just that the builder returns
        // a usable ServerConfig (rustls::ServerConfig has no public
        // getters to inspect, so we just assert it exists).
        let _ = rustls_cfg;
    }

    #[tokio::test]
    async fn load_rustls_config_errors_on_missing_cert() {
        ensure_crypto_provider();
        let cfg = TlsConfig {
            cert_pem_path: "/nonexistent/cert.pem".to_string(),
            key_pem_path: "tests/fixtures/test_tls_key.pem".to_string(),
        };
        let res = load_rustls_config(&cfg).await;
        assert!(res.is_err(), "missing cert should error");
        let msg = format!("{}", res.unwrap_err());
        assert!(
            msg.contains("cert.pem") || msg.contains("loading"),
            "error should mention cert: {msg}"
        );
    }

    #[tokio::test]
    async fn load_rustls_config_errors_on_missing_key() {
        ensure_crypto_provider();
        let cfg = TlsConfig {
            cert_pem_path: "tests/fixtures/test_tls_cert.pem".to_string(),
            key_pem_path: "/nonexistent/key.pem".to_string(),
        };
        let res = load_rustls_config(&cfg).await;
        assert!(res.is_err(), "missing key should error");
    }

    /// Integration: bind HTTPS with a self-signed cert, connect over TLS,
    /// verify a 200 OK + the HSTS header. The HSTS header is added by
    /// `api::router` via `security_headers_middleware`, so we attach the
    /// same middleware in the test to verify the contract end-to-end.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn https_server_responds_with_security_headers() {
        use axum::{middleware::from_fn, routing::get, Router};
        use axum_server::Handle;
        ensure_crypto_provider();

        let cfg = fixture_paths();

        async fn add_security_headers(
            req: axum::extract::Request,
            next: axum::middleware::Next,
        ) -> axum::response::Response {
            use axum::http::{header, HeaderValue};
            let mut resp = next.run(req).await;
            let h = resp.headers_mut();
            h.entry(header::STRICT_TRANSPORT_SECURITY)
                .or_insert(HeaderValue::from_static(
                    "max-age=31536000; includeSubDomains",
                ));
            h.entry("x-content-type-options")
                .or_insert(HeaderValue::from_static("nosniff"));
            h.entry("x-frame-options")
                .or_insert(HeaderValue::from_static("DENY"));
            h.entry(header::REFERRER_POLICY)
                .or_insert(HeaderValue::from_static("no-referrer"));
            h.entry(header::CONTENT_SECURITY_POLICY)
                .or_insert(HeaderValue::from_static("default-src 'self'"));
            resp
        }

        let app = Router::new()
            .route("/health", get(|| async { "ok" }))
            .layer(from_fn(add_security_headers));

        // Use Handle so we can wait for the OS to assign an ephemeral
        // port, then learn which port was actually bound.
        let handle = Handle::new();
        let rustls_cfg = load_rustls_config(&cfg).await.unwrap();
        let server = axum_server::bind_rustls("127.0.0.1:0".parse().unwrap(), rustls_cfg)
            .handle(handle.clone());
        let _task = tokio::spawn(async move {
            let _ = server.serve(app.into_make_service()).await;
        });
        let addr = handle
            .listening()
            .await
            .expect("server should publish bound addr");

        // Wrap the whole client dance in a 5s timeout.
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            // Connect with rustls client (skip cert verification — it's self-signed).
            let rc_config = ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(std::sync::Arc::new(NoCertificateVerification))
                .with_no_client_auth();
            let connector = TlsConnector::from(std::sync::Arc::new(rc_config));
            let stream = tokio::net::TcpStream::connect(addr).await?;
            let mut tls_stream = connector.connect("localhost".try_into()?, stream).await?;

            // Send a minimal HTTP/1.1 GET request over the TLS stream.
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            tls_stream
                .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .await?;

            let mut buf = Vec::new();
            tls_stream.read_to_end(&mut buf).await?;
            let response = String::from_utf8_lossy(&buf);
            eprintln!("TLS response:\n{response}");

            anyhow::ensure!(
                response.starts_with("HTTP/1.1 200 OK"),
                "expected 200 OK, got: {}",
                response.lines().next().unwrap_or("?")
            );
            anyhow::ensure!(
                response.contains("strict-transport-security"),
                "expected HSTS header in response, got:\n{response}"
            );
            anyhow::ensure!(
                response.contains("max-age=31536000"),
                "expected HSTS max-age=31536000, got:\n{response}"
            );
            Ok(())
        })
        .await;

        result
            .map_err(|_| anyhow::anyhow!("TLS roundtrip timed out after 5s"))
            .and_then(|inner| inner)
            .expect("TLS roundtrip failed");
    }

    /// Certificate verifier that accepts any cert (for self-signed tests).
    #[derive(Debug)]
    struct NoCertificateVerification;
    impl rustls::client::danger::ServerCertVerifier for NoCertificateVerification {
        fn verify_server_cert(
            &self,
            _end_entity: &rustls::pki_types::CertificateDer<'_>,
            _intermediates: &[rustls::pki_types::CertificateDer<'_>],
            _server_name: &rustls::pki_types::ServerName<'_>,
            _ocsp_response: &[u8],
            _now: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &rustls::pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &rustls::pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            rustls::crypto::aws_lc_rs::default_provider()
                .signature_verification_algorithms
                .supported_schemes()
        }
    }
}
