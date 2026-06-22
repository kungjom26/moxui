# Installation Guide

> **MoxUI** — Modern, secure Rust-based web UI for Proxmox VE

## Prerequisites

| Requirement | Version | Notes |
|---|---|---|
| Proxmox VE | 7.x / 8.x | Any node with API access (port 8006) |
| Rust (for building from source) | 1.78+ | Install via [rustup](https://rustup.rs/) |
| Docker (for container deployment) | 24.0+ | Or Podman |
| Kubernetes (for Helm deploy) | 1.27+ | With cert-manager optional |

---

## Option 1: Pre-built Binary (GitHub Releases)

> _Pre-built binaries are published on the [GitHub Releases](https://github.com/kungjom26/moxui/releases) page._

```bash
# Download the latest release for your architecture
curl -LO https://github.com/kungjom26/moxui/releases/latest/download/moxui-x86_64-unknown-linux-gnu.tar.gz

# Extract and install
tar xzf moxui-x86_64-unknown-linux-gnu.tar.gz
sudo install -m 0755 moxui /usr/local/bin/moxui

# Create config directory and edit
sudo mkdir -p /etc/moxui
sudo cp config.example.yaml /etc/moxui/config.yaml
sudo $EDITOR /etc/moxui/config.yaml

# Run
moxui
```

### Systemd Service (Linux)

A hardened systemd unit is included in the release package:

```bash
# Install the systemd unit
sudo install -m 0644 contrib/moxui.service /usr/lib/systemd/system/moxui.service
sudo systemctl daemon-reload
sudo systemctl enable --now moxui.service
```

The unit runs as `moxui:moxui` with:
- `ProtectSystem=strict`
- `NoNewPrivileges=yes`
- `RestrictNamespaces=true`
- `MemoryDenyWriteExecute=true`
- `PrivateTmp=true`

---

## Option 2: Build from Source with Cargo

```bash
# Clone the repository
git clone https://github.com/kungjom26/moxui.git
cd moxui

# Build release binary (LTO + stripped + abort-on-panic)
cargo build --release

# The binary is at ./target/release/moxui
# Copy it to your PATH
sudo install -m 0755 target/release/moxui /usr/local/bin/moxui

# Set up config
sudo mkdir -p /etc/moxui
sudo cp config.example.yaml /etc/moxui/config.yaml
sudo $EDITOR /etc/moxui/config.yaml
```

### Generate JWT keys

MoxUI requires RSA keys for JWT signing:

```bash
# Generate a 2048-bit RSA private key
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out /etc/moxui/jwt_priv.pem

# Extract the public key
openssl pkey -in /etc/moxui/jwt_priv.pem -pubout -out /etc/moxui/jwt_pub.pem

# Restrict permissions
sudo chmod 640 /etc/moxui/jwt_priv.pem /etc/moxui/jwt_pub.pem
sudo chown root:moxui /etc/moxui/jwt_priv.pem /etc/moxui/jwt_pub.pem
```

---

## Option 3: Docker (Recommended)

```bash
# Pull the image
docker pull ghcr.io/kungjom26/moxui:latest

# Create a data volume and config
docker volume create moxui-data

# Run
docker run -d \
  --name moxui \
  -p 8080:8080 \
  -v moxui-data:/var/lib/moxui/data \
  -v /path/to/config.yaml:/etc/moxui/config.yaml:ro \
  -v /path/to/jwt_priv.pem:/etc/moxui/jwt_priv.pem:ro \
  -v /path/to/jwt_pub.pem:/etc/moxui/jwt_pub.pem:ro \
  ghcr.io/kungjom26/moxui:latest
```

Access `http://localhost:8080`.

### Docker Compose

```yaml
# docker-compose.yml
version: "3.8"
services:
  moxui:
    image: ghcr.io/kungjom26/moxui:latest
    ports:
      - "8080:8080"
    volumes:
      - ./config.yaml:/etc/moxui/config.yaml:ro
      - ./jwt_priv.pem:/etc/moxui/jwt_priv.pem:ro
      - ./jwt_pub.pem:/etc/moxui/jwt_pub.pem:ro
      - moxui-data:/var/lib/moxui/data
    healthcheck:
      test: ["CMD", "wget", "--no-verbose", "--tries=1", "--spider", "http://localhost:8080/livez"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 10s
    restart: unless-stopped

volumes:
  moxui-data:
```

---

## Option 4: Debian Package

```bash
# Build the .deb package (requires dpkg-deb + fakeroot)
make package-deb

# Install
sudo dpkg -i moxui_*.deb

# Edit config
sudo nano /etc/moxui/config.yaml

# Enable and start
sudo systemctl enable --now moxui.service
```

---

## Quick Verification

After starting MoxUI, verify it's running:

```bash
# Liveness check (always 200 if process is alive)
curl http://localhost:8080/livez

# Health endpoint (detailed JSON)
curl http://localhost:8080/health

# Readiness check (pings Proxmox clusters)
curl http://localhost:8080/readyz
```

---

## Environment Variables

All config fields can be overridden via `MOXUI_*` environment variables:

| Variable | Default | Description |
|---|---|---|
| `MOXUI_CONFIG` | `/etc/moxui/config.yaml` | Config file path |
| `MOXUI_LOG_LEVEL` | `info` | Log level override |
| `MOXUI_SERVER__BIND` | `0.0.0.0:8080` | Bind address |
| `MOXUI_DATABASE__PATH` | `moxui.db` | SQLite database path |
| `MOXUI_AUTH__JWT_ISSUER` | `moxui` | JWT issuer |
| `MOXUI_AUTH__JWT_AUDIENCE` | `moxui-clients` | JWT audience |
| `MOXUI_AUTH__JWT_LIFETIME_SECS` | `3600` | JWT TTL (1h) |

Use double-underscore (`__`) for nested fields, e.g.:
```bash
export MOXUI_SERVER__BIND="0.0.0.0:9090"
export MOXUI_LOGGING__LEVEL="debug"
export MOXUI_AUTH__RATE_LIMIT__REQUESTS_PER_SECOND="10"
```
