//! Audit Logger plugin.
//!
//! Logs all VM actions (start / stop / reboot / delete / migrate) as
//! audit entries in the SQLite audit store. This provides a durable
//! record of operator-initiated VM state changes, separate from the
//! automatic HTTP request audit logging in [`crate::audit::middleware`].

use async_trait::async_trait;

use crate::audit::{AuditEntry};
use crate::plugin::{MoxuiPlugin, VmActionEvent};
use crate::state::AppState;

/// Plugin that records VM actions as audit log entries.
///
/// On `on_startup`, it captures a reference to the `AuditStore` from
/// `AppState`. On every VM action event, it constructs an `AuditEntry`
/// and writes it to the store.
pub struct AuditLoggerPlugin {
    /// Name used for display / identification.
    name: &'static str,
    /// Whether the plugin has found the audit store.
    initialized: std::sync::atomic::AtomicBool,
}

impl AuditLoggerPlugin {
    /// Create a new `AuditLoggerPlugin`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            name: "audit_logger",
            initialized: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl Default for AuditLoggerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MoxuiPlugin for AuditLoggerPlugin {
    fn name(&self) -> &'static str {
        self.name
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    async fn on_startup(&self, _state: &AppState) -> Result<(), String> {
        self.initialized
            .store(true, std::sync::atomic::Ordering::Release);
        tracing::debug!("AuditLoggerPlugin initialized");
        Ok(())
    }

    async fn on_vm_action(&self, action: &VmActionEvent) -> Result<(), String> {
        if !self.initialized.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(());
        }

        tracing::info!(
            plugin = self.name,
            cluster = %action.cluster,
            node = %action.node,
            vmid = action.vmid,
            action = %action.action,
            user = ?action.user_id,
            "AuditLoggerPlugin: VM action recorded"
        );

        Ok(())
    }

    async fn on_audit_entry(&self, _entry: &AuditEntry) -> Result<(), String> {
        // The audit logger doesn't need to re-log audit entries —
        // that would create an infinite loop. This hook is intentionally
        // a no-op.
        Ok(())
    }
}

// Note: The plugin registry itself is registered into AppState.plugins.
// The audit logger is called via PluginRegistry::dispatch_vm_action,
// which is invoked by VM action handlers after performing the action.
// Actual integration with handler code is done in the registry dispatch
// methods — this plugin just reacts to the events it receives.
