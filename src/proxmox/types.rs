//! Strongly-typed Proxmox API responses.
//!
//! Each struct corresponds to a Proxmox API endpoint response.
//! See `https://pve.proxmox.com/pve-docs/api-viewer/` for the raw JSON.

use serde::{Deserialize, Serialize};

/// Proxmox version info (from `/version`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    /// Proxmox VE version (e.g. `8.2.4`).
    pub version: String,
    /// Proxmox VE release (e.g. `8.2`).
    pub release: String,
    /// Repository ID the package was built from.
    pub repoid: String,
}

/// Node status (from `/nodes/{node}/status`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStatus {
    /// Node hostname.
    pub node: String,
    /// Node status (e.g. `online`, `offline`, `unknown`).
    pub status: String,
    /// Current CPU usage as a fraction in `[0.0, 1.0]`.
    #[serde(default)]
    pub cpu: Option<f64>,
    /// Support level (e.g. `basic`, `enterprise`).
    #[serde(default)]
    pub level: Option<String>,
    /// Total number of logical CPU cores.
    #[serde(default)]
    pub maxcpu: Option<u32>,
    /// Used memory in bytes.
    #[serde(default)]
    pub mem: Option<u64>,
    /// Total memory in bytes.
    #[serde(default)]
    pub maxmem: Option<u64>,
    /// Used root disk space in bytes.
    #[serde(default)]
    pub disk: Option<u64>,
    /// Total root disk space in bytes.
    #[serde(default)]
    pub maxdisk: Option<u64>,
    /// Node uptime in seconds.
    #[serde(default)]
    pub uptime: Option<u64>,
}

/// VM/LXC resource entry (from `/cluster/resources?type=vm`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmResource {
    /// Virtual machine ID (unique within a cluster).
    pub vmid: u32,
    /// VM/LXC name.
    pub name: String,
    /// Node that currently hosts the VM.
    pub node: String,
    /// Current status (e.g. `running`, `stopped`, `paused`).
    pub status: String,
    /// Current CPU usage as a fraction in `[0.0, 1.0]`.
    #[serde(default)]
    pub cpu: Option<f64>,
    /// Allocated CPU cores.
    #[serde(default)]
    pub cpus: Option<f64>,
    /// Used memory in bytes.
    #[serde(default)]
    pub mem: Option<u64>,
    /// Configured memory in bytes.
    #[serde(default)]
    pub maxmem: Option<u64>,
    /// Used root disk in bytes.
    #[serde(default)]
    pub disk: Option<u64>,
    /// Configured root disk in bytes.
    #[serde(default)]
    pub maxdisk: Option<u64>,
    /// Total network ingress in bytes.
    #[serde(default)]
    pub netin: Option<u64>,
    /// Total network egress in bytes.
    #[serde(default)]
    pub netout: Option<u64>,
    /// Total bytes read from disk.
    #[serde(default)]
    pub diskread: Option<u64>,
    /// Total bytes written to disk.
    #[serde(default)]
    pub diskwrite: Option<u64>,
    /// Uptime in seconds.
    #[serde(default)]
    pub uptime: Option<u64>,
    /// `1` if this entry is a template, `0` otherwise.
    #[serde(default)]
    pub template: Option<u8>,
    /// Semicolon-separated tags (e.g. `prod;web`).
    #[serde(default)]
    pub tags: Option<String>,
}

/// Standard Proxmox API response wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    /// Response payload.
    pub data: T,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_version() {
        let json = r#"{"data":{"version":"8.2.4","release":"8.2","repoid":"abc123"}}"#;
        let resp: ApiResponse<Version> = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.version, "8.2.4");
        assert_eq!(resp.data.release, "8.2");
    }

    #[test]
    fn test_deserialize_vm_resource() {
        let json = r#"{
            "data": {
                "vmid": 103,
                "name": "test-vm",
                "node": "pve11",
                "status": "running",
                "cpu": 0.12,
                "cpus": 2.0,
                "mem": 1073741824,
                "maxmem": 2147483648,
                "disk": 12884901888,
                "maxdisk": 21474836480,
                "uptime": 3600,
                "tags": "prod;web"
            }
        }"#;
        let resp: ApiResponse<VmResource> = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.vmid, 103);
        assert_eq!(resp.data.name, "test-vm");
        assert_eq!(resp.data.status, "running");
        assert!(resp.data.tags.is_some());
    }
}
