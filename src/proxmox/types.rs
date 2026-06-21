//! Strongly-typed Proxmox API responses.
//!
//! Each struct corresponds to a Proxmox API endpoint response.
//! See `https://pve.proxmox.com/pve-docs/api-viewer/` for the raw JSON.

use serde::{Deserialize, Serialize};

/// QEMU VM configuration (from `/nodes/{node}/qemu/{vmid}/config`).
///
/// This is the editable VM spec — cores, memory, disks, NICs, boot
/// order, etc. We only model the fields the operator UI surfaces;
/// Proxmox returns many more (smbios, hookscript, audio, etc.) that
/// we silently ignore via `#[serde(default)]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmConfig {
    /// VM name (the `name` property in the config).
    #[serde(default)]
    pub name: Option<String>,
    /// Description / notes.
    #[serde(default)]
    pub description: Option<String>,
    /// Allocated vCPU cores.
    #[serde(default)]
    pub cores: Option<u32>,
    /// Allocated vCPU sockets.
    #[serde(default)]
    pub sockets: Option<u32>,
    /// Configured memory in MiB (Proxmox uses MiB for memory fields).
    #[serde(default)]
    pub memory: Option<u64>,
    /// Configured ballooning minimum in MiB (0 = disabled).
    #[serde(default)]
    pub balloon: Option<u64>,
    /// Boot order (e.g. `order=scsi0;net0`).
    #[serde(default)]
    pub boot: Option<String>,
    /// BIOS type (`seabios` or `ovmf`).
    #[serde(default)]
    pub bios: Option<String>,
    /// Machine type (e.g. `pc-q35-8.1`).
    #[serde(default)]
    pub machine: Option<String>,
    /// SCSI controller model (e.g. `virtio-scsi-pci`).
    #[serde(default)]
    pub scsihw: Option<String>,
    /// CPU type (e.g. `host`, `kvm64`, `x86-64-v2-AES`).
    #[serde(default)]
    pub cpu: Option<String>,
    /// Free-form tags (semicolon-separated).
    #[serde(default)]
    pub tags: Option<String>,
    /// Whether the VM is a template (`1` = yes).
    #[serde(default)]
    pub template: Option<u8>,
    /// Onboot flag (`1` = start with host).
    #[serde(default)]
    pub onboot: Option<u8>,
    /// Agent flag (`1` = QEMU guest agent enabled).
    #[serde(default)]
    pub agent: Option<u8>,
}

/// Proxmox async task status (from `/nodes/{node}/tasks/{upid}/status`).
///
/// Tasks are how Proxmox reports state-changing operations (clone,
/// migrate, snapshot, backup, …). A `start`/`stop`/etc. on a VM
/// returns an `UPID` immediately; the actual work runs async and
/// callers poll this endpoint to know when it finishes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatus {
    /// Unique task ID (the same UPID returned by the action).
    pub upid: String,
    /// Node that owns the task (from the URL, not the UPID).
    #[serde(default)]
    pub node: Option<String>,
    /// Task status: `running`, `stopped` (finished OK), or `error`.
    /// Other states exist (`unknown`) but the UI treats them as in-progress.
    pub status: String,
    /// Human-readable exit status when `status == "stopped"`.
    #[serde(default)]
    pub exitstatus: Option<String>,
    /// Task start time, Unix seconds.
    #[serde(default)]
    pub starttime: Option<u64>,
    /// Task end time, Unix seconds (0 if still running).
    #[serde(default)]
    pub endtime: Option<u64>,
    /// Free-form type identifier (e.g. `qmstart`, `qmstop`, `qmdestroy`).
    #[serde(default)]
    pub r#type: Option<String>,
    /// User that initiated the task.
    #[serde(default)]
    pub user: Option<String>,
}

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
    /// Resource kind (`qemu` or `lxc`).
    ///
    /// Proxmox `/cluster/resources?type=vm` returns both VMs and containers
    /// in a single payload; we filter on this field in the client.
    #[serde(rename = "type", default)]
    pub kind: String,
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

/// Single LXC container status snapshot (from
/// `/nodes/{node}/lxc/{vmid}/status/current`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LxcStatus {
    /// Container ID.
    pub vmid: u32,
    /// Container name.
    pub name: String,
    /// Node that hosts the container.
    pub node: String,
    /// Current status (e.g. `running`, `stopped`).
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
    /// Used root filesystem in bytes.
    #[serde(default)]
    pub disk: Option<u64>,
    /// Configured root filesystem in bytes.
    #[serde(default)]
    pub maxdisk: Option<u64>,
    /// Uptime in seconds.
    #[serde(default)]
    pub uptime: Option<u64>,
    /// Container template flag.
    #[serde(default)]
    pub template: Option<u8>,
}

/// Storage pool summary (from `/nodes/{node}/storage`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageResource {
    /// Storage identifier (e.g. `local`, `ceph-pool`).
    pub storage: String,
    /// Storage type (e.g. `dir`, `zfspool`, `rbd`, `nfs`, `cifs`).
    #[serde(rename = "type")]
    pub kind: String,
    /// Total size in bytes (`0` for non-block storages like ISO-only dirs).
    #[serde(default)]
    pub total: u64,
    /// Used size in bytes.
    #[serde(default)]
    pub used: u64,
    /// Available size in bytes.
    #[serde(default)]
    pub avail: u64,
    /// Usage as a fraction in `[0.0, 1.0]`.
    #[serde(default)]
    pub used_fraction: Option<f64>,
    /// Whether this storage is enabled.
    #[serde(default)]
    pub enabled: Option<u8>,
    /// Whether this storage is shared across the cluster.
    #[serde(default)]
    pub shared: Option<u8>,
    /// Human-readable content types (e.g. `images,rootdir,iso,vztmpl`).
    #[serde(default)]
    pub content: Option<String>,
}

/// A single volume stored inside a storage pool (from
/// `/nodes/{node}/storage/{storage}/content`).
///
/// Examples: ISO images, container templates, VM disk images, backups.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageContent {
    /// Volume identifier (e.g. `local:iso/debian-12.iso`).
    pub volid: String,
    /// Storage holding this volume.
    pub storage: String,
    /// Content kind (e.g. `iso`, `vztmpl`, `images`, `backup`).
    pub content: String,
    /// Filename portion of `volid`.
    #[serde(default)]
    pub volid_name: Option<String>,
    /// Volume size in bytes.
    #[serde(default)]
    pub size: u64,
    /// Used bytes (e.g. for `images` after thin/thick allocation).
    #[serde(default)]
    pub used: Option<u64>,
    /// Volume format (e.g. `qcow2`, `raw`, `iso`, `tgz`).
    #[serde(default)]
    pub format: Option<String>,
    /// Creation time, Unix seconds.
    #[serde(default)]
    pub ctime: Option<u64>,
}

/// A network interface on a Proxmox node (from `/nodes/{node}/network`).
///
/// Proxmox returns bridges, bonds, VLANs, physical NICs, and Linux
/// aliases as a single flat list, with the `type` field distinguishing
/// them. The fields below cover the common subset; Proxmox returns
/// more fields per type that we don't model yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeNetwork {
    /// Interface name (e.g. `vmbr0`, `eno1`, `bond0`, `wlan0`).
    pub iface: String,
    /// Interface type (`bridge`, `bond`, `eth`, `vlan`, `alias`, `OVSBridge`).
    #[serde(rename = "type")]
    pub kind: String,
    /// Active flag (`1` = up, `0` = down). Older Proxmox versions
    /// reported this as a numeric; newer versions may report `"active"`
    /// in the parent object — we only model the integer form here.
    #[serde(default)]
    pub active: Option<u8>,
    /// IPv4 address with CIDR (e.g. `10.10.11.11/24`).
    #[serde(default)]
    pub address: Option<String>,
    /// IPv4 gateway.
    #[serde(default)]
    pub gateway: Option<String>,
    /// IPv6 address with CIDR.
    #[serde(default)]
    pub address6: Option<String>,
    /// IPv6 gateway.
    #[serde(default)]
    pub gateway6: Option<String>,
    /// For `bridge`: which physical interfaces are attached.
    #[serde(default)]
    pub bridge_ports: Option<String>,
    /// For `vlan`: the underlying raw interface (e.g. `eno1` for `eno1.10`).
    #[serde(default)]
    pub iface_vlan_raw_device: Option<String>,
    /// For `vlan`: the VLAN tag (e.g. `10` for `eno1.10`).
    #[serde(default)]
    pub vlan_id: Option<u32>,
    /// Autostart on boot (`1` = yes, `0` = no).
    #[serde(default)]
    pub autostart: Option<u8>,
    /// Comments / description from the Proxmox UI.
    #[serde(default)]
    pub comments: Option<String>,
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
