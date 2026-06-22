package provider

import (
	"context"
	"fmt"
	"net/http"
	"strconv"
	"strings"

	"github.com/hashicorp/terraform-plugin-sdk/v2/diag"
	"github.com/hashicorp/terraform-plugin-sdk/v2/helper/schema"
)

// VM represents a Proxmox QEMU virtual machine managed via MoxUI.
type VM struct {
	Cluster string `json:"cluster"`
	Node    string `json:"node"`
	Name    string `json:"name"`
	VMID    int    `json:"vmid"`
	Cores   int    `json:"cores"`
	Memory  int    `json:"memory"`
	Disk    string `json:"disk"`
	Status  string `json:"status,omitempty"`
}

func resourceVM() *schema.Resource {
	return &schema.Resource{
		CreateContext: resourceVMCreate,
		ReadContext:   resourceVMRead,
		UpdateContext: resourceVMUpdate,
		DeleteContext: resourceVMDelete,

		Schema: map[string]*schema.Schema{
			"cluster": {
				Type:        schema.TypeString,
				Required:    true,
				Description: "Proxmox cluster name",
			},
			"node": {
				Type:        schema.TypeString,
				Required:    true,
				Description: "Proxmox node name",
			},
			"name": {
				Type:        schema.TypeString,
				Required:    true,
				Description: "VM name",
			},
			"vmid": {
				Type:        schema.TypeInt,
				Required:    true,
				ForceNew:    true,
				Description: "Virtual machine ID (unique per cluster)",
			},
			"cores": {
				Type:        schema.TypeInt,
				Required:    true,
				Description: "Number of CPU cores",
			},
			"memory": {
				Type:        schema.TypeInt,
				Required:    true,
				Description: "Memory in MB",
			},
			"disk": {
				Type:        schema.TypeString,
				Required:    true,
				Description: "Disk storage identifier (e.g., local-lvm:32)",
			},
			"status": {
				Type:        schema.TypeString,
				Computed:    true,
				Description: "VM power status from MoxUI",
			},
		},

		Importer: &schema.ResourceImporter{
			StateContext: schema.ImportStatePassthroughContext,
		},
	}
}

func resourceVMCreate(ctx context.Context, d *schema.ResourceData, m interface{}) diag.Diagnostics {
	client := m.(*MoxuiClient)

	vm := &VM{
		Cluster: d.Get("cluster").(string),
		Node:    d.Get("node").(string),
		Name:    d.Get("name").(string),
		VMID:    d.Get("vmid").(int),
		Cores:   d.Get("cores").(int),
		Memory:  d.Get("memory").(int),
		Disk:    d.Get("disk").(string),
	}

	// POST /api/v1/vms/{cluster}/{node}/{vmid} — create VM
	path := fmt.Sprintf("/api/v1/vms/%s/%s/%d", vm.Cluster, vm.Node, vm.VMID)
	type createReq struct {
		Name  string `json:"name"`
		Cores int    `json:"cores"`
		Memory int   `json:"memory"`
		Disk   string `json:"disk"`
	}
	body := createReq{Name: vm.Name, Cores: vm.Cores, Memory: vm.Memory, Disk: vm.Disk}

	var result struct{} // backend returns 200/201 on success
	if err := client.doRequest(ctx, "POST", path, body, &result); err != nil {
		return diag.Errorf("error creating VM: %s", err)
	}

	id := fmt.Sprintf("%s:%s:%d", vm.Cluster, vm.Node, vm.VMID)
	d.SetId(id)

	return resourceVMRead(ctx, d, m)
}

func resourceVMRead(ctx context.Context, d *schema.ResourceData, m interface{}) diag.Diagnostics {
	client := m.(*MoxuiClient)

	// Parse the resource ID (cluster:node:vmid)
	parts := strings.SplitN(d.Id(), ":", 3)
	if len(parts) != 3 {
		return diag.Errorf("invalid resource ID: %s (expected cluster:node:vmid)", d.Id())
	}

	cluster := parts[0]
	node := parts[1]
	vmid, err := strconv.Atoi(parts[2])
	if err != nil {
		return diag.Errorf("invalid VMID in resource ID %s: %s", d.Id(), err)
	}

	// GET /api/v1/vms/{cluster}/{vmid} — fetch VM detail
	path := fmt.Sprintf("/api/v1/vms/%s/%d", cluster, vmid)
	var vm VM
	if err := client.doRequest(ctx, "GET", path, nil, &vm); err != nil {
		// Check if the VM was deleted externally
		return diag.FromErr(err)
	}

	d.Set("cluster", vm.Cluster)
	d.Set("node", vm.Node)
	d.Set("name", vm.Name)
	d.Set("vmid", vm.VMID)
	d.Set("cores", vm.Cores)
	d.Set("memory", vm.Memory)
	d.Set("disk", vm.Disk)
	d.Set("status", vm.Status)

	return nil
}

func resourceVMUpdate(ctx context.Context, d *schema.ResourceData, m interface{}) diag.Diagnostics {
	client := m.(*MoxuiClient)
	cluster := d.Get("cluster").(string)
	node := d.Get("node").(string)
	vmid := d.Get("vmid").(int)

	// PUT /api/v1/vms/{cluster}/{node}/{vmid}/config — update VM config
	path := fmt.Sprintf("/api/v1/vms/%s/%s/%d/config", cluster, node, vmid)
	type updateReq struct {
		Name  *string `json:"name,omitempty"`
		Cores *int    `json:"cores,omitempty"`
		Memory *int   `json:"memory,omitempty"`
		Disk  *string `json:"disk,omitempty"`
	}

	var req updateReq
	if d.HasChange("name") {
		v := d.Get("name").(string)
		req.Name = &v
	}
	if d.HasChange("cores") {
		v := d.Get("cores").(int)
		req.Cores = &v
	}
	if d.HasChange("memory") {
		v := d.Get("memory").(int)
		req.Memory = &v
	}
	if d.HasChange("disk") {
		v := d.Get("disk").(string)
		req.Disk = &v
	}

	var result struct{}
	if err := client.doRequest(ctx, "PUT", path, req, &result); err != nil {
		return diag.Errorf("error updating VM: %s", err)
	}

	return resourceVMRead(ctx, d, m)
}

func resourceVMDelete(ctx context.Context, d *schema.ResourceData, m interface{}) diag.Diagnostics {
	client := m.(*MoxuiClient)
	cluster := d.Get("cluster").(string)
	node := d.Get("node").(string)
	vmid := d.Get("vmid").(int)

	// DELETE /api/v1/vms/{cluster}/{node}/{vmid} — delete VM
	path := fmt.Sprintf("/api/v1/vms/%s/%s/%d", cluster, node, vmid)
	req, err := http.NewRequestWithContext(ctx, "DELETE", client.Endpoint+path, nil)
	if err != nil {
		return diag.FromErr(err)
	}
	req.Header.Set("Authorization", "Bearer "+client.Token)

	resp, err := client.HTTPClient.Do(req)
	if err != nil {
		return diag.Errorf("error deleting VM: %s", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode >= 400 {
		return diag.Errorf("error deleting VM: HTTP %d", resp.StatusCode)
	}

	d.SetId("")
	return nil
}
