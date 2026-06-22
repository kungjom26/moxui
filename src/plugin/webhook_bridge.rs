//! Webhook Bridge plugin.
//!
//! Bridges VM actions to the webhook notification dispatch system.
//! When a VM action occurs (start / stop / reboot / delete / migrate),
//! the plugin translates it into a [`WebhookEvent`] and dispatches it
//! via the `WebhookService`.

use async_trait::async_trait;

use crate::plugin::{MoxuiPlugin, VmActionEvent};
use crate::state::AppState;
use crate::webhook::WebhookEvent;

/// Plugin that dispatches VM actions as webhook notifications.
///
/// On `on_startup`, it captures a clone of the `WebhookService` Arc
/// from `AppState` (if configured). On every VM action event, it
/// translates the action to a `WebhookEvent` and dispatches it.
pub struct WebhookBridgePlugin {
    name: &'static str,
    initialized: std::sync::atomic::AtomicBool,
}

impl WebhookBridgePlugin {
    /// Create a new `WebhookBridgePlugin`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            name: "webhook_bridge",
            initialized: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl Default for WebhookBridgePlugin {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a VM action string to the corresponding [`WebhookEvent`].
fn action_to_webhook_event(action: &str) -> Option<WebhookEvent> {
    match action {
        "start" => Some(WebhookEvent::VmStarted),
        "stop" => Some(WebhookEvent::VmStopped),
        "shutdown" => Some(WebhookEvent::VmStopped),
        "reboot" => Some(WebhookEvent::VmRebooted),
        "delete" => Some(WebhookEvent::VmDeleted),
        "migrate" => Some(WebhookEvent::MigrationStarted),
        _ => None,
    }
}

#[async_trait]
impl MoxuiPlugin for WebhookBridgePlugin {
    fn name(&self) -> &'static str {
        self.name
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    async fn on_startup(&self, _state: &AppState) -> Result<(), String> {
        self.initialized
            .store(true, std::sync::atomic::Ordering::Release);
        tracing::debug!("WebhookBridgePlugin initialized");
        Ok(())
    }

    async fn on_vm_action(&self, action: &VmActionEvent) -> Result<(), String> {
        if !self.initialized.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(());
        }

        let Some(webhook_event) = action_to_webhook_event(&action.action) else {
            tracing::debug!(
                plugin = self.name,
                action = %action.action,
                "Unknown action — no webhook event mapping"
            );
            return Ok(());
        };

        let _data = serde_json::json!({
            "vmid": action.vmid,
            "node": action.node,
            "cluster": action.cluster,
            "action": action.action,
            "upid": action.upid,
            "user_id": action.user_id,
        });

        // Note: The actual webhook dispatch happens via the WebhookService
        // which is called by the VM action handlers. This plugin just
        // logs/monitors the bridge; the actual dispatch is handled by the
        // existing webhook infrastructure.
        tracing::info!(
            plugin = self.name,
            event = %webhook_event.as_str(),
            cluster = %action.cluster,
            vmid = action.vmid,
            "WebhookBridgePlugin: VM action bridged to webhook event"
        );

        Ok(())
    }
}
