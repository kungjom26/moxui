# MoxUI Terraform Provider — Example Usage
#
# This file demonstrates creating a VM on a Proxmox cluster via MoxUI.
# Adjust variable values in terraform.tfvars or pass them via -var.

terraform {
  required_providers {
    moxui = {
      source  = "moxui/moxui"
      version = "~> 0.1.0"
    }
  }
}

provider "moxui" {
  endpoint = var.moxui_endpoint
  username = var.moxui_username
  password = var.moxui_password
}

resource "moxui_vm" "web_01" {
  cluster = var.pve_cluster
  node    = var.pve_node
  name    = "web-01"
  vmid    = var.vm_id
  cores   = var.vm_cores
  memory  = var.vm_memory
  disk    = var.vm_disk
}

output "vm_info" {
  value = {
    id     = moxui_vm.web_01.id
    name   = moxui_vm.web_01.name
    status = moxui_vm.web_01.status
    vmid   = moxui_vm.web_01.vmid
    node   = moxui_vm.web_01.node
  }
  description = "Details of the created VM"
}
