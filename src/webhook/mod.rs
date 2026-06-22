//! Webhook notification dispatch system.
//!
//! Sends HTTP POST notifications to configured endpoints when Proxmox
//! events occur (VM started, stopped, deleted, etc.). Supports:
//! - HMAC-SHA256 payload signing
//! - Exponential backoff retry
//! - Slack/Discord-formatted message helpers
//! - Background dispatch via `tokio::spawn`

pub mod dispatcher;

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::config::WebhookConfig;

/// Event types that can trigger webhook notifications.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WebhookEvent {
    /// A VM was started.
    VmStarted,
    /// A VM was stopped.
    VmStopped,
    /// A VM was rebooted.
    VmRebooted,
    /// A VM was deleted.
    VmDeleted,
    /// A migration was started.
    MigrationStarted,
    /// An audit log entry was created.
    AuditLogged,
    /// A user logged in.
    UserLogin,
    /// A user logged out.
    UserLogout,
}

impl WebhookEvent {
    /// Return the string representation as used in endpoint subscriptions.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::VmStarted => "VmStarted",
            Self::VmStopped => "VmStopped",
            Self::VmRebooted => "VmRebooted",
            Self::VmDeleted => "VmDeleted",
            Self::MigrationStarted => "MigrationStarted",
            Self::AuditLogged => "AuditLogged",
            Self::UserLogin => "UserLogin",
            Self::UserLogout => "UserLogout",
        }
    }
}

/// Background webhook dispatch service.
///
/// Holds the configured endpoints and spawns a background task per event
/// to deliver notifications concurrently.
pub struct WebhookService {
    /// Endpoint configurations (name, url, events, secret, max_retries).
    endpoints: Vec<WebhookEndpoint>,
    /// Whether dispatch is enabled globally.
    enabled: bool,
    /// Default timeout for HTTP requests.
    timeout: Duration,
    /// Default retry count.
    default_retries: u32,
    /// Background join handles (for lifecycle management).
    handles: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}

/// A single configured webhook endpoint with its subscription list.
#[derive(Debug, Clone)]
pub struct WebhookEndpoint {
    /// Human-readable name.
    pub name: String,
    /// Target URL.
    pub url: String,
    /// Subscribed event types. Empty = all events.
    pub events: Vec<WebhookEvent>,
    /// Optional HMAC secret for signing.
    pub secret: Option<String>,
    /// Max retries for this endpoint.
    pub max_retries: u32,
}

impl WebhookService {
    /// Create a new `WebhookService` from configuration.
    ///
    /// Returns `None` if webhook dispatch is disabled in config.
    #[must_use]
    pub fn from_config(config: &WebhookConfig) -> Option<Arc<Self>> {
        if !config.enabled || config.endpoints.is_empty() {
            info!("Webhook notifications disabled or no endpoints configured");
            return None;
        }

        let endpoints: Vec<WebhookEndpoint> = config
            .endpoints
            .iter()
            .map(|ep| {
                let events: Vec<WebhookEvent> = if ep.events.is_empty() {
                    vec![]
                } else {
                    ep.events
                        .iter()
                        .filter_map(|e| match e.as_str() {
                            "VmStarted" => Some(WebhookEvent::VmStarted),
                            "VmStopped" => Some(WebhookEvent::VmStopped),
                            "VmRebooted" => Some(WebhookEvent::VmRebooted),
                            "VmDeleted" => Some(WebhookEvent::VmDeleted),
                            "MigrationStarted" => Some(WebhookEvent::MigrationStarted),
                            "AuditLogged" => Some(WebhookEvent::AuditLogged),
                            "UserLogin" => Some(WebhookEvent::UserLogin),
                            "UserLogout" => Some(WebhookEvent::UserLogout),
                            other => {
                                warn!(event = %other, "Unknown webhook event type in config");
                                None
                            }
                        })
                        .collect()
                };
                WebhookEndpoint {
                    name: ep.name.clone(),
                    url: ep.url.clone(),
                    events,
                    secret: ep.secret.clone(),
                    max_retries: ep.max_retries.unwrap_or(config.retry_count),
                }
            })
            .collect();

        info!(
            count = endpoints.len(),
            "Webhook notifications enabled"
        );

        Some(Arc::new(Self {
            endpoints,
            enabled: true,
            timeout: Duration::from_secs(config.timeout_secs),
            default_retries: config.retry_count,
            handles: Arc::new(Mutex::new(Vec::new())),
        }))
    }

    /// Dispatch a webhook event to all subscribed endpoints.
    ///
    /// Spawns a background task per endpoint. Returns immediately.
    pub fn dispatch_event(
        self: &Arc<Self>,
        event: &WebhookEvent,
        cluster: &str,
        data: &serde_json::Value,
    ) {
        if !self.enabled {
            return;
        }

        let event_name = event.as_str().to_string();
        let cluster = cluster.to_string();
        let payload = serde_json::json!({
            "event": event_name,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "cluster": cluster,
            "data": data,
        });

        for endpoint in &self.endpoints {
            // Skip if endpoint has specific subscriptions and this event isn't one of them
            if !endpoint.events.is_empty() && !endpoint.events.contains(event) {
                continue;
            }

            let payload = payload.clone();
            let endpoint = endpoint.clone();
            let timeout = self.timeout;

            let handle = tokio::spawn(async move {
                dispatcher::deliver_with_retry(
                    &endpoint,
                    &payload,
                    timeout,
                    endpoint.max_retries,
                )
                .await;
            });

            // We spawn a handle that we don't await — fire-and-forget is intentional
            // for webhooks. Dropping the handle detaches the task.
            let _ = handle;
        }
    }

    /// Format an event payload as Slack-compatible message blocks.
    #[must_use]
    pub fn format_event_to_slack(
        event: &WebhookEvent,
        cluster: &str,
        data: &serde_json::Value,
    ) -> serde_json::Value {
        let emoji = Self::event_emoji(event);
        let title = format!("{} {} event on {}", emoji, event.as_str(), cluster);

        let fields = Self::extract_fields(data);

        serde_json::json!({
            "blocks": [
                {
                    "type": "header",
                    "text": {
                        "type": "plain_text",
                        "text": title,
                        "emoji": true
                    }
                },
                {
                    "type": "section",
                    "fields": fields
                },
                {
                    "type": "context",
                    "elements": [
                        {
                            "type": "mrkdwn",
                            "text": format!("Cluster: {} | Timestamp: {}", cluster, chrono::Utc::now().to_rfc3339())
                        }
                    ]
                }
            ]
        })
    }

    /// Format an event payload as Discord-compatible message.
    #[must_use]
    pub fn format_event_to_discord(
        event: &WebhookEvent,
        cluster: &str,
        data: &serde_json::Value,
    ) -> serde_json::Value {
        let emoji = Self::event_emoji(event);
        let title = format!("{} {} event on {}", emoji, event.as_str(), cluster);
        let description = format!(
            "Event: **{}**\nCluster: **{}**\nTimestamp: {}",
            event.as_str(),
            cluster,
            chrono::Utc::now().to_rfc3339()
        );

        let fields = Self::extract_fields(data);

        serde_json::json!({
            "embeds": [
                {
                    "title": title,
                    "description": description,
                    "color": Self::event_color(event),
                    "fields": fields,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }
            ]
        })
    }

    /// Emoji representation of a webhook event.
    fn event_emoji(event: &WebhookEvent) -> &'static str {
        match event {
            WebhookEvent::VmStarted => "▶️",
            WebhookEvent::VmStopped => "⏹️",
            WebhookEvent::VmRebooted => "🔄",
            WebhookEvent::VmDeleted => "🗑️",
            WebhookEvent::MigrationStarted => "📦",
            WebhookEvent::AuditLogged => "📝",
            WebhookEvent::UserLogin => "🔑",
            WebhookEvent::UserLogout => "🚪",
        }
    }

    /// Color code for Discord embeds.
    fn event_color(event: &WebhookEvent) -> u32 {
        match event {
            WebhookEvent::VmStarted => 0x00ff00,       // green
            WebhookEvent::VmStopped => 0xff0000,        // red
            WebhookEvent::VmRebooted => 0xffaa00,       // orange
            WebhookEvent::VmDeleted => 0xcc0000,        // dark red
            WebhookEvent::MigrationStarted => 0x0088ff, // blue
            WebhookEvent::AuditLogged => 0x888888,      // gray
            WebhookEvent::UserLogin => 0x00cc88,        // teal
            WebhookEvent::UserLogout => 0x888888,       // gray
        }
    }

    /// Extract key-value fields from the data payload for Slack/Discord formatting.
    fn extract_fields(data: &serde_json::Value) -> Vec<serde_json::Value> {
        let mut fields = Vec::new();
        if let Some(obj) = data.as_object() {
            for (key, value) in obj.iter().take(8) {
                let val_str = match value {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    _ => value.to_string(),
                };
                fields.push(serde_json::json!({
                    "name": key,
                    "value": val_str,
                    "inline": true
                }));
            }
        }
        fields
    }
}
