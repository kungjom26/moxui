# MoxUI Terraform Provider — Variables

variable "moxui_endpoint" {
  description = "MoxUI API endpoint URL"
  type        = string
  default     = "http://localhost:8080"
}

variable "moxui_username" {
  description = "MoxUI administrator username"
  type        = string
  sensitive   = true
}

variable "moxui_password" {
  description = "MoxUI administrator password"
  type        = string
  sensitive   = true
}

variable "pve_cluster" {
  description = "Proxmox cluster name"
  type        = string
  default     = "proxmox-dc1"
}

variable "pve_node" {
  description = "Proxmox node name"
  type        = string
  default     = "pve-01"
}

variable "vm_id" {
  description = "VMID for the new virtual machine"
  type        = number
  default     = 1001
}

variable "vm_cores" {
  description = "Number of CPU cores"
  type        = number
  default     = 2
}

variable "vm_memory" {
  description = "Memory in MB"
  type        = number
  default     = 2048
}

variable "vm_disk" {
  description = "Disk storage (e.g., local-lvm:32 for 32 GB on local-lvm)"
  type        = string
  default     = "local-lvm:32"
}
