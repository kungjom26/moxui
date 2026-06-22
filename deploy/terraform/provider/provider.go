package provider

import (
	"context"
	"fmt"
	"net/http"
	"time"

	"github.com/hashicorp/terraform-plugin-sdk/v2/diag"
	"github.com/hashicorp/terraform-plugin-sdk/v2/helper/schema"
)

// MoxuiClient handles communication with the MoxUI REST API.
type MoxuiClient struct {
	HTTPClient *http.Client
	Endpoint   string
	Token      string
}

// Login authenticates against the MoxUI API and stores a JWT token.
func (c *MoxuiClient) Login(ctx context.Context, username, password string) error {
	type loginReq struct {
		Username string `json:"username"`
		Password string `json:"password"`
	}
	type loginResp struct {
		Token string `json:"token"`
	}

	reqBody := loginReq{Username: username, Password: password}
	var resp loginResp

	if err := c.doRequest(ctx, "POST", "/api/v1/auth/login", reqBody, &resp); err != nil {
		return fmt.Errorf("moxui login failed: %w", err)
	}

	if resp.Token == "" {
		return fmt.Errorf("moxui login returned empty token")
	}

	c.Token = resp.Token
	return nil
}

// doRequest performs an authenticated HTTP request against the MoxUI API.
func (c *MoxuiClient) doRequest(ctx context.Context, method, path string, body, result interface{}) error {
	// Simplified: in production, use proper JSON marshal/unmarshal
	// and handle HTTP transport errors.
	return nil // stub — full implementation in resource files
}

// Provider returns the Terraform provider resource map and configuration.
func Provider() *schema.Provider {
	return &schema.Provider{
		Schema: map[string]*schema.Schema{
			"endpoint": {
				Type:        schema.TypeString,
				Required:    true,
				DefaultFunc: schema.EnvDefaultFunc("MOXUI_ENDPOINT", "http://localhost:8080"),
				Description: "MoxUI API endpoint URL",
			},
			"username": {
				Type:        schema.TypeString,
				Required:    true,
				DefaultFunc: schema.EnvDefaultFunc("MOXUI_USERNAME", ""),
				Description: "MoxUI administrator username",
			},
			"password": {
				Type:        schema.TypeString,
				Required:    true,
				Sensitive:   true,
				DefaultFunc: schema.EnvDefaultFunc("MOXUI_PASSWORD", ""),
				Description: "MoxUI administrator password",
			},
		},

		ResourcesMap: map[string]*schema.Resource{
			"moxui_vm": resourceVM(),
		},

		ConfigureContextFunc: providerConfigure,
	}
}

func providerConfigure(ctx context.Context, d *schema.ResourceData) (interface{}, diag.Diagnostics) {
	endpoint := d.Get("endpoint").(string)
	username := d.Get("username").(string)
	password := d.Get("password").(string)

	// Validate inputs
	if endpoint == "" {
		return nil, diag.Errorf("moxui endpoint must not be empty")
	}
	if username == "" {
		return nil, diag.Errorf("moxui username must not be empty")
	}
	if password == "" {
		return nil, diag.Errorf("moxui password must not be empty")
	}

	client := &MoxuiClient{
		HTTPClient: &http.Client{
			Timeout: 30 * time.Second,
		},
		Endpoint: endpoint,
	}

	if err := client.Login(ctx, username, password); err != nil {
		return nil, diag.FromErr(err)
	}

	return client, nil
}
