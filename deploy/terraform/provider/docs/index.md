# MoxUI Terraform Provider — Documentation

## Overview

The MoxUI Terraform provider enables infrastructure-as-code management of Proxmox virtual machines through the MoxUI management plane. It abstracts the Proxmox REST API behind a Terraform-native resource model with JWT-based authentication.

## Provider Configuration

```hcl
provider "moxui" {
  endpoint = "https://moxui.example.com"
  username = "admin"
  password = var.moxui_password
}
```

### Environment Variables

| Variable | Description |
|----------|-------------|
| `MOXUI_ENDPOINT` | API endpoint URL |
| `MOXUI_USERNAME` | Administrator username |
| `MOXUI_PASSWORD` | Administrator password |

## Resources

### `moxui_vm`

Manages a QEMU/KVM VM on a Proxmox cluster.

**Create**: `POST /api/v1/vms/{cluster}/{node}/{vmid}`  
**Read**: `GET /api/v1/vms/{cluster}/{vmid}`  
**Update**: `PUT /api/v1/vms/{cluster}/{node}/{vmid}/config`  
**Delete**: `DELETE /api/v1/vms/{cluster}/{node}/{vmid}`  
**Import**: `terraform import moxui_vm.example <cluster>:<node>:<vmid>`

## Security

- All API calls use Bearer token authentication (JWT)
- Credentials are marked `sensitive` in Terraform schema
- Supports environment variable injection for CI/CD pipelines
