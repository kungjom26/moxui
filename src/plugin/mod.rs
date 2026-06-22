//! Plugin system for MoxUI.
//!
//! Provides a hook-based plugin trait ([`MoxuiPlugin`]) and registry
//! ([`PluginRegistry`]) that lets extensions observe and react to
//! MoxUI events without modifying core code.
//!
//! ## Architecture
//!
//! Plugins implement [`MoxuiPlugin`] and are registered at startup.
//! The [`PluginRegistry`] dispatches events to all registered plugins
//! in registration order. Each hook returns `Ok(())` on success or
//! `Err(String)` on failure; the registry logs failures but does not
//! abort the calling operation (best-effort).
//!
//! ## Example
//!
//! ```rust,ignore
//! use moxui::plugin::{MoxuiPlugin, VmActionEvent};
//!
//! struct MyPlugin;
//!
//! #[async_trait::async_trait]
//! impl MoxuiPlugin for MyPlugin {
//!     fn name(&self) -> &'static str { "my-plugin" }
//!     fn version(&self) -> &'static str { "0.1.0" }
//!
//!     async fn on_vm_action(&self, action: &VmActionEvent) -> Result<(), String> {
//!         tracing::info!("VM action: {:?}", action);
//!         Ok(())
//!     }
//! }
//! ```

pub mod audit_logger;
pub mod webhook_bridge;

use crate::audit::AuditEntry;
use crate::state::AppState;

/// Event payload for a VM action (start / stop / shutdown / reboot / delete / migrate).
#[derive(Debug, Clone)]
pub struct VmActionEvent {
    /// Cluster name where the action was performed.
    pub cluster: String,
    /// Node hosting the VM.
    pub node: String,
    /// VMID the action was applied to.
    pub vmid: u32,
    /// Action name (e.g. `"start"`, `"stop"`, `"reboot"`, `"delete"`, `"migrate"`).
    pub action: String,
    /// Proxmox UPID returned by the action.
    pub upid: String,
    /// Authenticated user that triggered the action, if known.
    pub user_id: Option<String>,
}

/// A plugin that can hook into MoxUI events.
///
/// All hooks default to `Ok(())` — implement only the ones you need.
/// Hooks are called **synchronously** in the hot path and MUST NOT
/// block for significant durations; use `tokio::spawn` inside your
/// hook if you need background work.
#[async_trait::async_trait]
pub trait MoxuiPlugin: Send + Sync {
    /// Unique plugin name (e.g. `"audit-logger"`).
    fn name(&self) -> &'static str;

    /// Plugin version (e.g. `"0.1.0"`).
    fn version(&self) -> &'static str;

    /// Called when a VM action is performed.
    async fn on_vm_action(&self, _action: &VmActionEvent) -> Result<(), String> {
        Ok(())
    }

    /// Called when an audit entry is logged.
    async fn on_audit_entry(&self, _entry: &AuditEntry) -> Result<(), String> {
        Ok(())
    }

    /// Called on startup — plugins can initialize state here.
    async fn on_startup(&self, _state: &AppState) -> Result<(), String> {
        Ok(())
    }
}

/// Registry of loaded plugins.
///
/// Dispatches events to all registered plugins. Failures are logged
/// but never propagated to callers — plugins are best-effort observers.
///
/// Note: This type does not implement `Clone`. It is wrapped in
/// `Arc<PluginRegistry>` inside [`AppState`] for shared access.
pub struct PluginRegistry {
    plugins: Vec<Box<dyn MoxuiPlugin>>,
}

impl PluginRegistry {
    /// Create a new empty plugin registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Register a plugin.
    ///
    /// Plugins are called in registration order for each event.
    pub fn register(&mut self, plugin: Box<dyn MoxuiPlugin>) {
        self.plugins.push(plugin);
    }

    /// Dispatch a VM action event to all registered plugins.
    ///
    /// Errors from individual plugins are logged via `tracing::warn!`.
    pub async fn dispatch_vm_action(&self, action: &VmActionEvent) {
        for plugin in self.plugins.iter() {
            if let Err(e) = plugin.on_vm_action(action).await {
                tracing::warn!(
                    plugin = plugin.name(),
                    error = %e,
                    "Plugin on_vm_action failed"
                );
            }
        }
    }

    /// Dispatch an audit entry to all registered plugins.
    ///
    /// Errors from individual plugins are logged via `tracing::warn!`.
    pub async fn dispatch_audit_entry(&self, entry: &AuditEntry) {
        for plugin in self.plugins.iter() {
            if let Err(e) = plugin.on_audit_entry(entry).await {
                tracing::warn!(
                    plugin = plugin.name(),
                    error = %e,
                    "Plugin on_audit_entry failed"
                );
            }
        }
    }

    /// Notify all plugins that the application has started.
    ///
    /// Called once at startup via [`PluginRegistry::init_plugins`].
    /// Errors are logged — a failing `on_startup` hook does not
    /// prevent the application from running.
    pub async fn dispatch_startup(&self, state: &AppState) {
        for plugin in self.plugins.iter() {
            tracing::info!(
                plugin = plugin.name(),
                version = plugin.version(),
                "Calling plugin on_startup"
            );
            if let Err(e) = plugin.on_startup(state).await {
                tracing::warn!(
                    plugin = plugin.name(),
                    error = %e,
                    "Plugin on_startup failed"
                );
            }
        }
    }

    /// Returns the number of registered plugins.
    #[must_use]
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Returns `true` if no plugins are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Return an iterator over plugin names.
    pub fn names(&self) -> Vec<&'static str> {
        self.plugins.iter().map(|p| p.name()).collect()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the [`PluginRegistry`] from a list of enabled plugin names.
///
/// Each name is matched against known built-in plugins:
/// - `"audit_logger"` — logs all VM actions to the audit store
/// - `"webhook_bridge"` — bridges VM actions to the webhook service
///
/// Unknown names are logged as warnings and skipped.
pub fn build_plugin_registry(
    enabled: &[String],
) -> PluginRegistry {
    let mut registry = PluginRegistry::new();

    for name in enabled {
        match name.as_str() {
            "audit_logger" => {
                registry.register(Box::new(audit_logger::AuditLoggerPlugin::new()));
                tracing::info!("Plugin 'audit_logger' registered");
            }
            "webhook_bridge" => {
                registry.register(Box::new(webhook_bridge::WebhookBridgePlugin::new()));
                tracing::info!("Plugin 'webhook_bridge' registered");
            }
            other => {
                tracing::warn!(plugin = %other, "Unknown plugin name in config — skipping");
            }
        }
    }

    registry
}
