//! Strongly-typed Proxmox API responses.
//!
//! Each struct corresponds to a Proxmox API endpoint response.
//! See `https://pve.proxmox.com/pve-docs/api-viewer/` for the raw JSON.

use secrecy::SecretString;
use serde::{Deserialize, Serialize};

/// One-shot VNC proxy ticket + port (from `POST /nodes/{node}/qemu/{vmid}/vncproxy`).
///
/// The ticket is consumed by the next WebSocket connection and must
/// not be reused or logged. We wrap both fields in `SecretString` so
/// that accidental Debug logging doesn't leak them — `tracing` and
/// `panic` messages both honor `Debug` and would otherwise print the
/// raw `PVEVNC:...` token.
#[derive(Debug)]
pub struct VncProxyTicket {
    /// Short-lived ticket string (e.g. `PVEVNC:...`). One-shot.
    pub ticket: SecretString,
    /// TCP port to connect the VNC WebSocket to (relative to the node).
    pub port: u16,
    /// UPID of the vncproxy task (kept for audit / log correlation;
    /// ticket itself is what we hand to the WS upgrade).
    pub upid: String,
}

impl Clone for VncProxyTicket {
    fn clone(&self) -> Self {
        use secrecy::ExposeSecret;
        Self {
            ticket: SecretString::new(self.ticket.expose_secret().to_string().into_boxed_str()),
            port: self.port,
            upid: self.upid.clone(),
        }
    }
}

// serde::Deserialize for the wire format — we read the raw fields
// from Proxmox and immediately box them into SecretString so the
// plaintext only lives in SecretString's heap allocation.
impl<'de> Deserialize<'de> for VncProxyTicket {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Raw {
            ticket: String,
            port: u16,
            upid: String,
        }
        let raw = Raw::deserialize(deserializer)?;
        Ok(VncProxyTicket {
            ticket: SecretString::new(raw.ticket.into_boxed_str()),
            port: raw.port,
            upid: raw.upid,
        })
    }
}

// We don't implement Serialize — these never go back out over JSON
// (the ticket is forwarded to the WS proxy in-process). The compile
// error if anything tries to serialize acts as a useful guardrail.
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

/// A replication job from Proxmox `/cluster/replication`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationJob {
    /// Replication job ID.
    pub id: u64,
    /// Whether the job is enabled.
    #[serde(default)]
    pub enable: Option<u8>,
    /// Source node.
    #[serde(default)]
    pub source_node: Option<String>,
    /// Source VM ID.
    #[serde(default)]
    pub source_vmid: Option<u32>,
    /// Target node (Proxmox remote target identifier).
    #[serde(default)]
    pub target: Option<String>,
    /// Target VM ID.
    #[serde(default)]
    pub target_vmid: Option<u32>,
    /// Replication rate limit in MB/s (0 = unlimited).
    #[serde(default)]
    pub rate: Option<u32>,
    /// Replication schedule in standard cron format.
    #[serde(default)]
    pub schedule: Option<String>,
    /// Free-form comment.
    #[serde(default)]
    pub comment: Option<String>,
    /// Job type (e.g. `local`, `remote`).
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
}

/// Replication job status (from `/cluster/replication/{id}/log` or status endpoint).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationStatus {
    /// Replication job ID.
    pub id: u64,
    /// Current state (e.g. `running`, `stopped`, `error`, `pending`).
    #[serde(default)]
    pub state: Option<String>,
    /// Timestamp of last successful sync (Unix seconds).
    #[serde(default)]
    pub last_sync: Option<u64>,
    /// Timestamp of last attempt (Unix seconds).
    #[serde(default)]
    pub last_try: Option<u64>,
    /// Duration of last run in seconds.
    #[serde(default)]
    pub duration: Option<u64>,
    /// Error message if last run failed.
    #[serde(default)]
    pub error: Option<String>,
    /// Total bytes replicated.
    #[serde(default)]
    pub bytes: Option<u64>,
}

/// Request body for creating a replication job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateReplicationJob {
    /// Source VM ID.
    pub source_vmid: u32,
    /// Source node.
    #[serde(default)]
    pub source_node: Option<String>,
    /// Target node/remote identifier.
    pub target: String,
    /// Target VM ID (defaults to source_vmid if not set).
    #[serde(default)]
    pub target_vmid: Option<u32>,
    /// Replication rate limit in MB/s.
    #[serde(default)]
    pub rate: Option<u32>,
    /// Schedule in cron format.
    pub schedule: String,
    /// Free-form comment.
    #[serde(default)]
    pub comment: Option<String>,
    /// Enable immediately.
    #[serde(default = "default_enable")]
    pub enable: bool,
}

fn default_enable() -> bool {
    true
}

/// Request body for updating an existing replication job.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateReplicationJob {
    /// Whether the job is enabled.
    #[serde(default)]
    pub enable: Option<bool>,
    /// Replication rate limit in MB/s.
    #[serde(default)]
    pub rate: Option<u32>,
    /// Schedule in cron format.
    #[serde(default)]
    pub schedule: Option<String>,
    /// Free-form comment.
    #[serde(default)]
    pub comment: Option<String>,
}

/// Proxmox HA group (from `/cluster/ha/groups`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HaGroup {
    /// HA group name/identifier.
    pub group: String,
    /// Comma-separated list of allowed nodes or autofailback target.
    #[serde(default)]
    pub nodes: Option<String>,
    /// Group type (e.g. `"basic"`, `"crs"`, etc.).
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    /// Free-form comment/description.
    #[serde(default)]
    pub comment: Option<String>,
    /// Whether to fail back to the preferred node automatically (`1` = yes, `0` = no).
    #[serde(default)]
    pub nofailback: Option<u8>,
    /// Whether VMs in this group cannot start on non-restricted nodes (`1` = yes, `0` = no).
    #[serde(default)]
    pub restricted: Option<u8>,
}

// ── Batch 1: VM/LXC/Storage Write Operations ──────────────────────────────

/// Request body for creating a VM.
///
/// Proxmox endpoint: `POST /nodes/{node}/qemu`.
/// The `vmid` field is optional; Proxmox will auto-assign one if omitted.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateVmRequest {
    /// VM ID (optional — auto-assigned if omitted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vmid: Option<u32>,
    /// VM name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// VM description / notes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Allocated vCPU cores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cores: Option<u32>,
    /// Allocated vCPU sockets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sockets: Option<u32>,
    /// Memory size in MiB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<u64>,
    /// Memory ballooning minimum in MiB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balloon: Option<u64>,
    /// Boot order (e.g. `order=scsi0;net0`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boot: Option<String>,
    /// BIOS type (`seabios` or `ovmf`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bios: Option<String>,
    /// Machine type (e.g. `pc-q35-8.1`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine: Option<String>,
    /// SCSI controller model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scsihw: Option<String>,
    /// CPU type (e.g. `host`, `kvm64`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<String>,
    /// CPU architecture (`x86_64`, `aarch64`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    /// Tags (semicolon-separated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    /// Whether this VM is a template (`1` = yes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<u8>,
    /// Start on boot (`1` = yes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onboot: Option<u8>,
    /// QEMU guest agent (`1` = enabled).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<u8>,
    /// Storage pool for the VM disk (e.g. `local-lvm`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<String>,
    /// Disk image size (e.g. `32G`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_size: Option<String>,
    /// Network model for the default NIC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net_model: Option<String>,
    /// Bridge for the default NIC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net_bridge: Option<String>,
    /// OS type (e.g. `l26` for Linux 2.6+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ostype: Option<String>,
    /// IDE2 media (CDROM image).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ide2: Option<String>,
    /// Start VM after creation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<bool>,
    /// Sockets × cores = total vCPUs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numa_enabled: Option<bool>,
}

/// Request body for cloning a VM.
///
/// Proxmox endpoint: `POST /nodes/{node}/qemu/{vmid}/clone`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneVmRequest {
    /// New VM ID (required).
    pub newid: u32,
    /// Target node (defaults to source node).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// New VM name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Target storage for the clone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<String>,
    /// Whether to create a full clone (default: true for linked clones).
    #[serde(default = "default_full_clone")]
    pub full: bool,
    /// Snapshot name to clone from (if not current state).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapname: Option<String>,
    /// VM description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether to start the cloned VM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<bool>,
    /// Pool to add the VM to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool: Option<String>,
}

fn default_full_clone() -> bool {
    true
}

/// Request body for updating a VM's configuration.
///
/// Proxmox endpoint: `PUT /nodes/{node}/qemu/{vmid}/config`.
/// Only set fields will be updated; `None` fields are ignored.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateVmConfigRequest {
    /// VM name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// VM description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Allocated vCPU cores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cores: Option<u32>,
    /// Allocated vCPU sockets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sockets: Option<u32>,
    /// Memory size in MiB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<u64>,
    /// Memory ballooning minimum in MiB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balloon: Option<u64>,
    /// Boot order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boot: Option<String>,
    /// BIOS type (`seabios` or `ovmf`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bios: Option<String>,
    /// Machine type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine: Option<String>,
    /// SCSI controller model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scsihw: Option<String>,
    /// CPU type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<String>,
    /// Tags (semicolon-separated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    /// Template flag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<u8>,
    /// Start on boot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onboot: Option<u8>,
    /// QEMU guest agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<u8>,
    /// Delete specified config keys (comma-separated list of keys to reset to default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delete: Option<String>,
    /// Cloud-init CIUSER (ssh username).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ciuser: Option<String>,
    /// Cloud-init SSH public keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sshkeys: Option<String>,
    /// Cloud-init IP config (e.g. `ip=dhcp` or `ip=10.0.0.2/24,gw=10.0.0.1`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ipconfig: Option<String>,
    /// Nameservers for cloud-init.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nameserver: Option<String>,
    /// Search domain for cloud-init.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub searchdomain: Option<String>,
    /// Network model override for specific NIC (e.g. `virtio` for `net0`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net: Option<String>,
}

/// A single VM snapshot entry.
///
/// Proxmox endpoint: `GET /nodes/{node}/qemu/{vmid}/snapshot`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotEntry {
    /// Snapshot name.
    pub name: String,
    /// Snapshot description.
    #[serde(default)]
    pub description: Option<String>,
    /// Snapshot creation time (Unix seconds).
    #[serde(default)]
    pub snaptime: Option<u64>,
    /// VM configuration at snapshot time (serialized).
    #[serde(default)]
    pub vmstate: Option<u8>,
    /// Whether this is the current (running) snapshot marker.
    #[serde(default)]
    pub parent: Option<String>,
}

/// A single backup file entry (parsed from storage content listing).
///
/// Proxmox stores backup files on storage; this represents one
/// backup artifact for a given VM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupEntry {
    /// Volume identifier (e.g. `local:backup/vzdump-qemu-100-2024_01_01-00_00_00.vma.zst`).
    pub volid: String,
    /// Storage name.
    pub storage: String,
    /// Backup filename.
    #[serde(default)]
    pub filename: Option<String>,
    /// File size in bytes.
    #[serde(default)]
    pub size: u64,
    /// Backup format (e.g. `vma`, `vma.zst`, `vma.gz`, `tar`).
    #[serde(default)]
    pub format: Option<String>,
    /// Creation time (Unix seconds).
    #[serde(default)]
    pub ctime: Option<u64>,
    /// Backup notes / comment.
    #[serde(default)]
    pub notes: Option<String>,
}

/// Request body for triggering a VM backup.
///
/// Proxmox endpoint: `POST /nodes/{node}/qemu/{vmid}/backup`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackupVmRequest {
    /// Target storage for the backup.
    #[serde(default = "default_backup_storage")]
    pub storage: String,
    /// Backup mode (`snapshot`, `suspend`, `stop`).
    #[serde(default = "default_backup_mode")]
    pub mode: String,
    /// Compression (`zstd`, `gzip`, `lzo`, `none`).
    #[serde(default = "default_backup_compress")]
    pub compress: String,
    /// Whether to remove the backup from the VM's backup list after completion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remove: Option<bool>,
    /// Notes / comment for the backup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Perform a standard backup (not a template backup).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performance: Option<String>,
}

fn default_backup_storage() -> String {
    "local".to_string()
}
fn default_backup_mode() -> String {
    "snapshot".to_string()
}
fn default_backup_compress() -> String {
    "zstd".to_string()
}

/// Request body for creating a VM snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSnapshotRequest {
    /// Snapshot name (required).
    pub snapname: String,
    /// Snapshot description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether to save the VM's memory state (default: true).
    #[serde(default = "default_vmstate")]
    pub vmstate: bool,
}

fn default_vmstate() -> bool {
    true
}

/// Request body for deleting a VM snapshot.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeleteSnapshotRequest {
    /// Force removal even if the VM is running.
    #[serde(default)]
    pub force: bool,
}

/// Request body for disk resize operations.
///
/// Proxmox endpoint: `POST /nodes/{node}/qemu/{vmid}/resize`.
/// Supports both regular disks (scsi0, virtio0, etc.) and cloudinit drives.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResizeDiskRequest {
    /// Disk identifier (e.g. `scsi0`, `virtio0`, `ide0`, `cloudinit`).
    pub disk: String,
    /// Size to add/subtract (e.g. `+10G`, `-5G`, `32G` for absolute).
    /// Use `+` prefix to grow, `-` to shrink. Absolute values set target size.
    pub size: String,
}

/// Request body for LXC action (start / stop / shutdown / reboot).
/// Typically no body is needed; the action is in the URL path.
/// This exists for consistency with the VM action pattern.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LxcActionRequest {
    /// Force the action (for stop/reboot, skip the graceful shutdown).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force: Option<bool>,
    /// Timeout in seconds before force-stopping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u32>,
}

/// Request body for deleting an LXC container.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeleteLxcRequest {
    /// Destroy and remove all volumes belonging to the container.
    #[serde(default)]
    pub purge: bool,
    /// Force deletion even if running.
    #[serde(default)]
    pub force: bool,
    /// Skip config lock.
    #[serde(default)]
    pub skiplock: bool,
}

/// Response from a storage upload operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadResponse {
    /// The uploaded volume identifier.
    pub volid: String,
    /// Status message.
    #[serde(default)]
    pub msg: Option<String>,
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
