# MoxUI Terraform Provider — Basic Example

This example demonstrates the minimal configuration needed to provision a VM through MoxUI using Terraform.

## Usage

```bash
# Set credentials
export MOXUI_ENDPOINT="http://localhost:8080"
export MOXUI_USERNAME="admin"
export MOXUI_PASSWORD="your-password"

# Initialize Terraform
terraform init

# Preview changes
terraform plan

# Apply
terraform apply

# Destroy
terraform destroy
```

## Files

- `main.tf` — provider config + `moxui_vm` resource definition
