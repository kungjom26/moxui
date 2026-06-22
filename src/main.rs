//! `MoxUI` binary entry point.
//!
//! See [`moxui::lib`] for the library API.

use std::process::ExitCode;

use clap::Parser;
use secrecy::SecretBox;

use moxui::audit::AuditStore;
use moxui::auth::webauthn::WebauthnState;
use moxui::auth::{JwtService, UserStore};
use moxui::config::Config;
use moxui::observability::metrics::{MetricsLayer, MetricsService};
use moxui::proxmox::ProxmoxClient;
use moxui::state::AppState;

#[derive(Parser, Debug)]
#[command(name = "moxui", version, about = "Modern Proxmox UI")]
struct Cli {
    /// Path to config file
    #[arg(short, long, default_value = "/etc/moxui/config.yaml")]
    config: String,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, env = "MOXUI_LOG_LEVEL", default_value = "info")]
    log_level: String,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    // Initialize tracing
    let logging_config = moxui::config::LoggingConfig {
        level: if cli.verbose {
            "debug".to_string()
        } else {
            cli.log_level.clone()
        },
        format: if cfg!(debug_assertions) {
            "pretty".to_string()
        } else {
            "json".to_string()
        },
    };

    if let Err(e) = moxui::telemetry::init(&logging_config) {
        eprintln!("Failed to initialize telemetry: {e}");
        return ExitCode::from(1);
    }

    // Install rustls crypto provider (required by rustls 0.23+)
    moxui::install_crypto_provider();

    // Load configuration
    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load config: {e}");
            return ExitCode::from(1);
        }
    };

    // Initialize OpenTelemetry tracing (OTLP) after config is loaded
    if let Err(e) = moxui::observability::tracing::init_tracing(&config.tracing) {
        tracing::warn!(error = %e, "Failed to initialize OpenTelemetry tracing — continuing without OTLP");
    }

    tracing::info!(
        version = moxui::VERSION,
        git_sha = moxui::GIT_SHA,
        "Starting MoxUI"
    );

    tracing::info!(
        bind = %config.server.bind,
        clusters = config.clusters.len(),
        "Configuration loaded"
    );

    // Create Proxmox clients (one per cluster) and audit insecure mode
    let mut clients = Vec::with_capacity(config.clusters.len());
    for cluster in &config.clusters {
        // C3 fix: audit log whenever insecure TLS is enabled. Production
        // should always use `ca_cert_pem` and `insecure_skip_verify: false`.
        if cluster.insecure_skip_verify {
            tracing::warn!(
                cluster = %cluster.name,
                "cluster has insecure_skip_verify=true — TLS validation disabled. \
                 Use ca_cert_pem in production."
            );
        }
        match ProxmoxClient::new(cluster.clone()).await {
            Ok(c) => clients.push(c),
            Err(e) => {
                tracing::error!(cluster = %cluster.name, error = %e, "Failed to build Proxmox client");
                return ExitCode::from(1);
            }
        }
    }
    tracing::info!(count = clients.len(), "Proxmox clients built");

    // Build JwtService (auth) and UserStore (seeded from config.auth.users)
    let jwt = match build_jwt_service(&config.auth) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "Failed to build JWT service");
            return ExitCode::from(1);
        }
    };
    let users = match build_user_store(&config.auth.users) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "Failed to build user store");
            return ExitCode::from(1);
        }
    };
    tracing::info!(
        users = users.len(),
        jwt_issuer = %config.auth.jwt_issuer,
        "Auth wired"
    );

    // Initialize audit log store (SQLite). Uses the configured DB path with
    // a `.audit` suffix so it sits next to the main DB but is easy to find.
    let audit_path = format!("{}.audit", config.database.path);
    let audit = match AuditStore::open(&audit_path) {
        Ok(s) => std::sync::Arc::new(s),
        Err(e) => {
            tracing::error!(error = %e, path = %audit_path, "Failed to open audit store");
            return ExitCode::from(1);
        }
    };
    tracing::info!(path = %audit_path, "Audit log store opened");

    // Load VNC HMAC secret (optional). When `auth.vnc_token_secret_pem_path`
    // is set we load the file as raw bytes and wrap it in `Secret` so
    // Debug/Display dumps never leak it. When unset, the VNC endpoints
    // become a 404 — that's the documented opt-out.
    let vnc_secret = match load_vnc_secret(&config.auth) {
        Ok(secret) => secret,
        Err(e) => {
            tracing::error!(error = %e, "Failed to load VNC secret");
            return ExitCode::from(1);
        }
    };
    if vnc_secret.is_some() {
        tracing::info!("VNC console enabled");
    } else {
        tracing::info!("VNC console disabled (auth.vnc_token_secret_pem_path not set)");
    }

    // Initialize WebAuthn / passkey support (optional)
    let webauthn_state = if config.auth.webauthn.enabled {
        match WebauthnState::new(
            &config.auth.webauthn.rp_id,
            &config.auth.webauthn.rp_origin,
            &config.auth.webauthn.rp_name,
        ) {
            Ok(ws) => {
                tracing::info!("WebAuthn enabled (rp_id={})", config.auth.webauthn.rp_id);
                Some(ws)
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "WebAuthn config invalid — passkey login disabled"
                );
                None
            }
        }
    } else {
        tracing::info!("WebAuthn disabled (auth.webauthn.enabled=false)");
        None
    };

    // Initialize OIDC / OAuth2 SSO (optional)
    let oidc_service = if config.auth.oidc.enabled {
        match moxui::auth::oidc::OidcService::new(&config.auth.oidc.providers).await {
            Ok(Some(svc)) => {
                tracing::info!(
                    providers = config.auth.oidc.providers.len(),
                    "OIDC / OAuth2 SSO enabled"
                );
                Some(svc)
            }
            Ok(None) => {
                tracing::info!("OIDC / OAuth2 SSO enabled but no providers configured");
                None
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to initialize OIDC / OAuth2 SSO");
                return ExitCode::from(1);
            }
        }
    } else {
        tracing::info!("OIDC / OAuth2 SSO disabled (auth.oidc.enabled=false)");
        None
    };

    // Initialize metrics service
    let metrics = MetricsService::new();
    tracing::info!("Prometheus metrics initialized");

    // Create application state
    let state = AppState::new(
        config.clone(),
        clients,
        audit,
        jwt,
        users,
        vnc_secret,
        webauthn_state,
        oidc_service,
        Some(metrics.clone()),
    );

    // Build router — API + UI are merged inside `api::router` so the
    // security-headers layer covers both with a single application.
    // UI serves `/` and `/static/*` (SPA shell + embedded assets) and
    // is public; auth still applies to `/api/v1/*` via the inner layer.
    let app = moxui::api::router(state).layer(MetricsLayer::new(metrics));

    // Bind and serve (HTTPS if server.tls is configured, plaintext HTTP otherwise)
    let bind = config.server.bind.clone();
    let addr: std::net::SocketAddr = match bind.parse() {
        Ok(a) => a,
        Err(e) => {
            tracing::error!(error = %e, bind = %bind, "Invalid bind address");
            return ExitCode::from(1);
        }
    };

    if let Err(e) = moxui::tls::serve(addr, app, config.server.tls.as_ref()).await {
        tracing::error!(error = %e, "Server error");
        return ExitCode::from(1);
    }

    // Gracefully shut down OpenTelemetry tracer
    moxui::observability::tracing::shutdown_tracing();

    ExitCode::SUCCESS
}

/// Build the [`JwtService`] from [`crate::config::AuthConfig`].
///
/// Reads RSA private + public keys from `jwt_private_key_pem_path` /
/// `jwt_public_key_pem_path`. If either is missing, **refuses to start**
/// (fail-closed) — running with auth-disabled would expose VM write
/// endpoints to anonymous callers.
#[allow(clippy::result_large_err)] // String is fine for startup config errors
fn build_jwt_service(auth: &moxui::config::AuthConfig) -> Result<JwtService, String> {
    let priv_path = auth
        .jwt_private_key_pem_path
        .as_ref()
        .ok_or_else(|| "auth.jwt_private_key_pem_path is required".to_string())?;
    let pub_path = auth
        .jwt_public_key_pem_path
        .as_ref()
        .ok_or_else(|| "auth.jwt_public_key_pem_path is required".to_string())?;
    let priv_pem = std::fs::read(priv_path).map_err(|e| format!("read {priv_path}: {e}"))?;
    let pub_pem = std::fs::read(pub_path).map_err(|e| format!("read {pub_path}: {e}"))?;
    JwtService::new(&priv_pem, &pub_pem, &auth.jwt_issuer, &auth.jwt_audience)
        .map_err(|e| format!("JWT key load: {e}"))
}

/// Build the [`UserStore`] from a list of [`crate::config::UserConfig`].
/// An empty `users` vec is allowed (no logins possible — all writes
/// return 401).
fn build_user_store(configs: &[moxui::config::UserConfig]) -> Result<UserStore, String> {
    UserStore::from_configs(configs)
}

/// Load the VNC HMAC secret from `auth.vnc_token_secret_pem_path`.
///
/// The file is read as raw bytes (we don't parse it as PEM — the
/// secret is opaque key material, not a certificate). Returns
/// `Ok(None)` when the path is unset (VNC disabled — endpoints will
/// respond 404). Returns `Err` when the path is set but the file is
/// missing/unreadable (fail-closed — half-configured VNC is worse
/// than no VNC).
///
/// We refuse to start when an empty file is loaded too: a zero-byte
/// secret is trivially brute-forceable and we don't want a silent
/// misconfiguration to weaken token signing.
fn load_vnc_secret(auth: &moxui::config::AuthConfig) -> Result<Option<SecretBox<Vec<u8>>>, String> {
    let Some(path) = auth.vnc_token_secret_pem_path.as_ref() else {
        return Ok(None);
    };
    let bytes = std::fs::read(path).map_err(|e| format!("read {path}: {e}"))?;
    if bytes.is_empty() {
        return Err(format!(
            "{path} is empty — refusing to use a zero-length VNC secret"
        ));
    }
    if bytes.len() < 32 {
        // 32 bytes = 256 bits, matches the HMAC-SHA256 block size.
        // We log the length but never the contents.
        tracing::warn!(
            path = %path,
            bytes = bytes.len(),
            "VNC secret is shorter than 32 bytes — HMAC-SHA256 is \
             still secure but consider a longer key for defense in depth"
        );
    }
    Ok(Some(SecretBox::new(Box::new(bytes))))
}
