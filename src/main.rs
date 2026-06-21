//! `MoxUI` binary entry point.
//!
//! See [`moxui::lib`] for the library API.

use std::process::ExitCode;

use clap::Parser;

use moxui::audit::AuditStore;
use moxui::auth::{JwtService, UserStore};
use moxui::config::Config;
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

    tracing::info!(
        version = moxui::VERSION,
        git_sha = moxui::GIT_SHA,
        "Starting MoxUI"
    );

    // Load configuration
    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "Failed to load config");
            return ExitCode::from(1);
        }
    };

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

    // Create application state
    let state = AppState::new(config.clone(), clients, audit, jwt, users);

    // Build router — API + UI are merged inside `api::router` so the
    // security-headers layer covers both with a single application.
    // UI serves `/` and `/static/*` (SPA shell + embedded assets) and
    // is public; auth still applies to `/api/v1/*` via the inner layer.
    let app = moxui::api::router(state);

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
