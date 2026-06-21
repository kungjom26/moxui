//! `MoxUI` binary entry point.
//!
//! See [`moxui::lib`] for the library API.

use std::process::ExitCode;

use clap::Parser;

use moxui::audit::AuditStore;
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
    let state = AppState::new(config.clone(), clients, audit);

    // Build router
    let app = moxui::api::router(state);

    // Bind and serve
    let bind = config.server.bind.clone();
    let listener = match tokio::net::TcpListener::bind(&bind).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(error = %e, bind = %bind, "Failed to bind");
            return ExitCode::from(1);
        }
    };

    tracing::info!(bind = %bind, "MoxUI listening");

    if let Err(e) = axum::serve(listener, app).await {
        tracing::error!(error = %e, "Server error");
        return ExitCode::from(1);
    }

    ExitCode::SUCCESS
}
