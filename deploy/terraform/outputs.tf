# MoxUI Terraform Provider — Outputs

output "vm_id" {
  value       = moxui_vm.web_01.id
  description = "Fully-qualified resource ID (cluster:node:vmid)"
}

output "vm_vmid" {
  value       = moxui_vm.web_01.vmid
  description = "Numeric VM ID on the Proxmox cluster"
}

output "vm_name" {
  value       = moxui_vm.web_01.name
  description = "VM display name"
}

output "vm_status" {
  value       = moxui_vm.web_01.status
  description = "Current power status of the VM"
}

output "vm_node" {
  value       = moxui_vm.web_01.node
  description = "Proxmox node hosting the VM"
}
