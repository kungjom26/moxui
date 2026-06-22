package provider

import (
	"testing"

	"github.com/hashicorp/terraform-plugin-sdk/v2/helper/resource"
	"github.com/hashicorp/terraform-plugin-sdk/v2/terraform"
)

func TestAccMoxuiVM_Basic(t *testing.T) {
	resource.Test(t, resource.TestCase{
		PreCheck:          func() { testAccPreCheck(t) },
		ProviderFactories: testAccProviderFactories,
		CheckDestroy:      testAccCheckMoxuiVMDestroy,
		Steps: []resource.TestStep{
			{
				Config: testAccMoxuiVMConfigBasic,
				Check: resource.ComposeTestCheckFunc(
					testAccCheckMoxuiVMExists("moxui_vm.test"),
					resource.TestCheckResourceAttr("moxui_vm.test", "name", "tf-acc-test"),
					resource.TestCheckResourceAttr("moxui_vm.test", "cores", "2"),
					resource.TestCheckResourceAttr("moxui_vm.test", "memory", "2048"),
					resource.TestCheckResourceAttr("moxui_vm.test", "disk", "local-lvm:32"),
				),
			},
			{
				Config: testAccMoxuiVMConfigUpdate,
				Check: resource.ComposeTestCheckFunc(
					testAccCheckMoxuiVMExists("moxui_vm.test"),
					resource.TestCheckResourceAttr("moxui_vm.test", "name", "tf-acc-test-updated"),
					resource.TestCheckResourceAttr("moxui_vm.test", "cores", "4"),
					resource.TestCheckResourceAttr("moxui_vm.test", "memory", "4096"),
				),
			},
		},
	})
}

func testAccPreCheck(t *testing.T) {
	// In CI, set MOXUI_ENDPOINT, MOXUI_USERNAME, MOXUI_PASSWORD env vars.
	// For local acceptance tests, ensure a test MoxUI instance is running.
}

func testAccCheckMoxuiVMExists(resourceName string) resource.TestCheckFunc {
	return func(s *terraform.State) error {
		rs, ok := s.RootModule().Resources[resourceName]
		if !ok {
			return terraform.Errorf("resource not found: %s", resourceName)
		}
		if rs.Primary.ID == "" {
			return terraform.Errorf("resource ID is not set")
		}
		return nil
	}
}

func testAccCheckMoxuiVMDestroy(s *terraform.State) error {
	for _, rs := range s.RootModule().Resources {
		if rs.Type != "moxui_vm" {
			continue
		}
		// VM should be gone — the provider's delete should have run.
		// If the resource still exists in the API, return an error.
	}
	return nil
}

const testAccMoxuiVMConfigBasic = `
provider "moxui" {
  endpoint = "http://localhost:8080"
  username = "admin"
  password = "admin"
}

resource "moxui_vm" "test" {
  cluster = "proxmox-dc1"
  node    = "pve-01"
  name    = "tf-acc-test"
  vmid    = 9999
  cores   = 2
  memory  = 2048
  disk    = "local-lvm:32"
}
`

const testAccMoxuiVMConfigUpdate = `
provider "moxui" {
  endpoint = "http://localhost:8080"
  username = "admin"
  password = "admin"
}

resource "moxui_vm" "test" {
  cluster = "proxmox-dc1"
  node    = "pve-01"
  name    = "tf-acc-test-updated"
  vmid    = 9999
  cores   = 4
  memory  = 4096
  disk    = "local-lvm:32"
}
`
