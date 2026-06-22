terraform {
  required_providers {
    moxui = {
      source  = "moxui/moxui"
      version = "~> 0.1.0"
    }
  }
}

provider "moxui" {
  # Uses MOXUI_ENDPOINT, MOXUI_USERNAME, MOXUI_PASSWORD env vars by default
}

resource "moxui_vm" "example" {
  cluster = "proxmox-dc1"
  node    = "pve-01"
  name    = "example-vm"
  vmid    = 1100
  cores   = 2
  memory  = 4096
  disk    = "local-lvm:40"
}

output "example_vm_id" {
  value = moxui_vm.example.id
}
