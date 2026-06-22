# MoxUI Terraform Provider

> Manage Proxmox VMs via MoxUI's REST API using Terraform.

## Overview

The MoxUI Terraform provider allows you to manage virtual machines on Proxmox clusters through MoxUI using standard Terraform workflows. It communicates with the MoxUI REST API using JWT-based authentication.

## Prerequisites

- Terraform >= 1.0
- A running MoxUI instance with JWT authentication enabled
- MoxUI API credentials (username/password)

## Usage

```hcl
provider "moxui" {
  endpoint = "https://moxui.example.com"
  username = "admin"
  password = var.moxui_password
}

resource "moxui_vm" "web_server" {
  cluster = "proxmox-dc1"
  node    = "pve-01"
  name    = "web-01"
  vmid    = 1001
  cores   = 4
  memory  = 8192
  disk    = "local-lvm:32"
}
```

## Resources

### `moxui_vm`

Manages a QEMU/KVM virtual machine on a Proxmox cluster via MoxUI.

**Arguments:**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| cluster  | string | yes | Proxmox cluster name |
| node     | string | yes | Proxmox node name |
| name     | string | yes | VM name |
| vmid     | number | yes | Unique VM ID |
| cores    | number | yes | Number of CPU cores |
| memory   | number | yes | Memory in MB |
| disk     | string | yes | Disk storage identifier (e.g., `local-lvm:32`) |

**Attributes:**

| Attribute | Type | Description |
|-----------|------|-------------|
| id        | string | Resource ID (`cluster:node:vmid`) |
| status    | string | VM power status |
| cluster   | string | Cluster name |
| node      | string | Node name |
| vmid      | number | VM ID |
| name      | string | VM name |
| cores     | number | CPU cores |
| memory    | number | Memory in MB |
| disk      | string | Disk identifier |

## Development

The provider is written in Go using the Terraform Plugin SDK v2. To build:

```bash
cd provider
go build -o terraform-provider-moxui
```

To install locally for testing:

```bash
mkdir -p ~/.terraform.d/plugins/registry.terraform.io/moxui/moxui/0.1.0/linux_amd64/
cp provider/terraform-provider-moxui ~/.terraform.d/plugins/registry.terraform.io/moxui/moxui/0.1.0/linux_amd64/
```
