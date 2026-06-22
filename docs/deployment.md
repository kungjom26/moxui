# Deployment Guide

> MoxUI — Production deployment options for running at scale

---

## Table of Contents

- [Docker / docker-compose](#docker--docker-compose)
- [Kubernetes / Helm](#kubernetes--helm)
- [Bare-Metal / Debian Package](#bare-metal--debian-package)
- [TLS / Reverse Proxy](#tls--reverse-proxy)
- [Production Checklist](#production-checklist)
- [Monitoring & Observability](#monitoring--observability)

---

## Docker / docker-compose

### Quick Start (docker run)

```bash
docker run -d \
  --name moxui \
  -p 8443:8080 \
  -v moxui-data:/var/lib/moxui/data \
  -v ./config.yaml:/etc/moxui/config.yaml:ro \
  -v ./jwt_priv.pem:/etc/moxui/jwt_priv.pem:ro \
  -v ./jwt_pub.pem:/etc/moxui/jwt_pub.pem:ro \
  -v ./cert.pem:/etc/moxui/tls/cert.pem:ro \
  -v ./key.pem:/etc/moxui/tls/key.pem:ro \
  ghcr.io/kungjom26/moxui:latest
```

### docker-compose

```yaml
# docker-compose.yml
version: "3.8"

services:
  moxui:
    image: ghcr.io/kungjom26/moxui:latest
    container_name: moxui
    restart: unless-stopped
    ports:
      - "8443:8080"
    volumes:
      - ./config.yaml:/etc/moxui/config.yaml:ro
      - ./jwt_priv.pem:/etc/moxui/jwt_priv.pem:ro
      - ./jwt_pub.pem:/etc/moxui/jwt_pub.pem:ro
      - ./tls:/etc/moxui/tls:ro
      - moxui_data:/var/lib/moxui/data
    environment:
      - MOXUI_DATABASE__PATH=/var/lib/moxui/data/moxui.db
      - TZ=UTC
    healthcheck:
      test: ["CMD", "wget", "--no-verbose", "--tries=1", "--spider", "http://localhost:8080/livez"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 10s

volumes:
  moxui_data:
```

### Building the Docker Image

```bash
# From project root
docker build -t moxui:latest .
```

The Dockerfile uses a multi-stage build:
1. **Builder stage** (`rust:1.78-slim-bookworm`) — compiles the binary with LTO + strip
2. **Runtime stage** (`debian:bookworm-slim`) — minimal image with CA certs + non-root user

---

## Kubernetes / Helm

A Helm chart is included at `deploy/k8s/moxui/`.

### Quick Install

```bash
# From project root
helm upgrade --install moxui ./deploy/k8s/moxui \
  --namespace moxui \
  --create-namespace \
  --set secrets.jwtPrivateKey="$(cat jwt_priv.pem)" \
  --set secrets.jwtPublicKey="$(cat jwt_pub.pem)" \
  --set config.clusters[0].name=homelab \
  --set config.clusters[0].url=https://192.168.1.11:8006 \
  --set config.clusters[0].username=root@pam \
  --set config.clusters[0].realm=pam
```

### Chart Values

| Parameter | Default | Description |
|---|---|---|
| `image.repository` | `ghcr.io/kungjom26/moxui` | Container image |
| `image.tag` | `latest` | Image tag |
| `image.pullPolicy` | `Always` | Pull policy |
| `replicaCount` | `2` | Number of replicas |
| `service.type` | `ClusterIP` | Service type |
| `service.port` | `8080` | Service port |
| `ingress.enabled` | `true` | Enable ingress |
| `ingress.host` | `moxui.example.com` | Ingress hostname |
| `config.*` | (see values.yaml) | MoxUI configuration |
| `clusters[]` | — | Proxmox cluster list |
| `existingSecret` | `""` | Reference existing K8s Secret |
| `secrets.*` | — | Auto-generated Secret values |
| `serviceMonitor.enabled` | `true` | Prometheus ServiceMonitor |
| `autoscaling.enabled` | `true` | HPA (2-10 replicas) |
| `pdb.enabled` | `true` | PodDisruptionBudget (min 1) |
| `networkPolicy.enabled` | `true` | NetworkPolicy |
| `persistence.enabled` | `true` | PVC for SQLite (1Gi) |
| `resources.requests` | `250m CPU / 256Mi RAM` | Resource requests |
| `resources.limits` | `1000m CPU / 512Mi RAM` | Resource limits |

### Key Templates

- **`deployment.yaml`** — Main deployment with probes, security context, volumes
- **`configmap.yaml`** — MoxUI YAML config rendered from values
- **`secret.yaml`** — JWT keys, cluster passwords, OIDC secrets (auto-generated or referenced)
- **`ingress.yaml`** — Ingress with TLS support
- **`servicemonitor.yaml`** — Prometheus scrape config
- **`hpa.yaml`** — CPU/memory-based autoscaling
- **`pdb.yaml`** — Pod disruption budget
- **`networkpolicy.yaml`** — Network ingress/egress rules
- **`pvc.yaml`** — Persistent volume claim for SQLite data

---

## Bare-Metal / Debian Package

### Building a .deb Package

```bash
# Requires: cargo, dpkg-deb, fakeroot
make package-deb
# Produces: moxui_0.1.0_amd64.deb
```

### Installing from .deb

```bash
sudo dpkg -i moxui_0.1.0_amd64.deb
sudo systemctl daemon-reload
sudo systemctl enable --now moxui.service
```

The package installs:
- Binary: `/usr/bin/moxui`
- systemd unit: `/usr/lib/systemd/system/moxui.service`
- Example config: `/etc/moxui/config.yaml.example`
- Documentation: `/usr/share/doc/moxui/`

### Manual Install

```bash
make install
# Installs to PREFIX (default /usr/local)
```

### systemd Unit

The hardened systemd unit (packaged in contrib/ or installed via `make install`) runs as `moxui:moxui` with security hardening:

```ini
[Unit]
Description=MoxUI — Modern Proxmox Web UI
After=network.target

[Service]
Type=simple
User=moxui
Group=moxui
ExecStart=/usr/bin/moxui
Restart=on-failure
RestartSec=5

# Security hardening
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
NoNewPrivileges=yes
MemoryDenyWriteExecute=yes
PrivateDevices=yes
RestrictNamespaces=yes
RestrictRealtime=yes
SystemCallArchitectures=native
CapabilityBoundingSet=
```

---

## TLS / Reverse Proxy

### Option A: Built-in TLS (axum-server + rustls)

Configure in `config.yaml`:

```yaml
server:
  bind: "0.0.0.0:8443"
  tls:
    cert_pem_path: "/etc/moxui/tls/cert.pem"
    key_pem_path: "/etc/moxui/tls/key.pem"
```

Certificates must be PEM-encoded full chains (leaf + intermediates). The key must be unencrypted PKCS#8 or RSA PEM.

### Option B: Reverse Proxy with Caddy (Recommended)

```caddyfile
moxui.example.com {
    reverse_proxy localhost:8080
    header / Strict-Transport-Security "max-age=31536000; includeSubDomains"
    header / X-Content-Type-Options "nosniff"
    header / X-Frame-Options "DENY"
}
```

### Option C: Reverse Proxy with nginx

```nginx
server {
    listen 443 ssl http2;
    server_name moxui.example.com;

    ssl_certificate     /etc/ssl/certs/moxui.crt;
    ssl_certificate_key /etc/ssl/private/moxui.key;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # WebSocket support for VNC console
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

---

## Production Checklist

- [ ] **TLS enabled** — Either MoxUI's built-in TLS or a reverse proxy
- [ ] **JWT keys** — RS256 key pair generated (`openssl genrsa -out jwt_priv.pem 2048`)
- [ ] **Strong passwords** — bcrypt hashes in config, env vars for secrets
- [ ] **insecure_skip_verify: false** — Use `ca_cert_pem` instead
- [ ] **rate_limit configured** — Prevent brute force on login
- [ ] **CORS restricted** — Set `allowed_origins` to your domain(s)
- [ ] **Logging format: json** — Structured logs for log aggregation
- [ ] **VNC secret** — Set `vnc_token_secret_pem_path` (32+ random bytes)
- [ ] **Backups** — SQLite database at `{path}` and `{path}.audit`
- [ ] **Monitoring** — Prometheus `/metrics` + health checks configured
- [ ] **Updates** — Watch releases for security patches

---

## Monitoring & Observability

### Prometheus Metrics

MoxUI exposes Prometheus metrics at `GET /metrics`:

```yaml
# Prometheus scrape config
scrape_configs:
  - job_name: 'moxui'
    static_configs:
      - targets: ['moxui:8080']
    metrics_path: /metrics
```

A Kubernetes ServiceMonitor is included in the Helm chart.

### Health Endpoints

| Endpoint | Purpose | Returns |
|---|---|---|
| `GET /health` | Detailed health | JSON with version, git SHA, uptime |
| `GET /livez` | Kubernetes liveness | `200 OK` (always, if process is alive) |
| `GET /readyz` | Kubernetes readiness | `200 OK` if all clusters reachable, `503` otherwise |

Readiness probes cache results for 10 seconds (configurable in code).

### Structured Logging

In production, set `logging.format: json` for log aggregation:

```json
{"timestamp":"2026-06-22T12:00:00Z","level":"INFO","message":"Starting MoxUI","version":"0.1.0","git_sha":"abc123","cluster":"homelab"}
```

### OpenTelemetry Tracing

When `tracing.enabled` is true, spans are exported via OTLP gRPC:

```yaml
tracing:
  enabled: true
  otlp_endpoint: "http://otel-collector:4317"
  service_name: "moxui"
```

Supports any OpenTelemetry-compatible backend (Jaeger, Grafana Tempo, SigNoz, etc.).

---

## Environment Variables in Production

```bash
# Core
MOXUI_CONFIG=/etc/moxui/config.yaml
MOXUI_SERVER__BIND=0.0.0.0:8080
MOXUI_LOGGING__LEVEL=info
MOXUI_DATABASE__PATH=/var/lib/moxui/data/moxui.db

# Secrets (never in config files)
MOXUI_AUTH__JWT_PRIVATE_KEY_PEM_PATH=/run/secrets/jwt_priv.pem
MOXUI_AUTH__JWT_PUBLIC_KEY_PEM_PATH=/run/secrets/jwt_pub.pem
MOXUI_AUTH__VNC_TOKEN_SECRET_PEM_PATH=/run/secrets/vnc_secret.bin
MOXUI_AUTH__API_KEY__KEY=<shared-api-key>

# Proxmox cluster passwords (one per cluster)
MOXUI_PROXMOX_HOMELAB_PASSWORD=<password>
MOXUI_PROXMOX_PROD_PASSWORD=<password>
```
