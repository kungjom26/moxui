# Installing moxui

MoxUI is distributed as a single static binary, an optional `.deb` package,
or as a container image. The binary has no runtime dependencies beyond
`libc` and standard CA certificates.

## Option 1: Debian / Ubuntu (.deb)

The `.deb` package installs the binary, systemd unit, and a sample
config. It also creates the `moxui` system user.

```bash
# Build the package locally
make package-deb
sudo dpkg -i moxui_0.1.0_amd64.deb

# Edit the config — set cluster URLs + passwords
sudo install -d -m 0750 /etc/moxui
sudo cp /etc/moxui/config.yaml.example /etc/moxui/config.yaml
sudo chmod 0640 /etc/moxui/config.yaml
sudoedit /etc/moxui/config.yaml

# Generate JWT signing keys
sudo openssl genrsa -out /etc/moxui/jwt_priv.pem 4096
sudo openssl rsa -in /etc/moxui/jwt_priv.pem -pubout -out /etc/moxui/jwt_pub.pem
sudo chown moxui:moxui /etc/moxui/jwt_*.pem
sudo chmod 0640 /etc/moxui/jwt_*.pem

# Start the service
sudo systemctl daemon-reload
sudo systemctl enable --now moxui.service

# Check it's running
sudo systemctl status moxui
curl -k https://localhost:8080/health
```

## Option 2: Manual install (any distro)

```bash
# Build a release binary
make build-release

# Install (default prefix: /usr/local)
sudo make install

# Create the moxui system user
sudo useradd --system --home /var/lib/moxui --shell /usr/sbin/nologin moxui
sudo install -d -m 0750 -o moxui -g moxui /var/lib/moxui /var/log/moxui

# Wire up the systemd unit
sudo systemctl daemon-reload
sudo systemctl enable --now moxui.service
```

## Option 3: From source (no install)

For a quick local run during development:

```bash
# Generate JWT keys
mkdir -p .dev-keys
openssl genrsa -out .dev-keys/jwt_priv.pem 2048
openssl rsa -in .dev-keys/jwt_priv.pem -pubout -out .dev-keys/jwt_pub.pem

# Copy + edit the sample config
cp contrib/moxui.yaml.example .dev/config.yaml
$EDITOR .dev/config.yaml

# Run
MOXUI_CONFIG=$(pwd)/.dev/config.yaml cargo run --release
```

## Generating TLS certs

MoxUI requires TLS in production (HTTPS-only mode per spec §6.2). For
self-signed dev/test certs:

```bash
openssl req -x509 -newkey rsa:2048 -nodes \
  -keyout /etc/moxui/tls/key.pem \
  -out /etc/moxui/tls/cert.pem \
  -days 36500 -subj "/CN=$(hostname -f)" \
  -addext "subjectAltName=DNS:$(hostname -f),IP:$(hostname -I | awk '{print $1}')"
```

For production, use a public CA (Let's Encrypt) or your internal CA.
Point `server.tls.cert_pem_path` + `server.tls.key_pem_path` at the
files and restart.

## Unattended upgrades

The `.deb` package declares `/etc/moxui/config.yaml.example` as a
conffile — your edits to a real `/etc/moxui/config.yaml` will not be
overwritten by `apt upgrade`. The package will only update the
`.example` template.

## Reverse proxy

If you want to terminate TLS at a reverse proxy (nginx, Caddy, Traefik)
instead of having moxui do it directly, leave `server.tls` unconfigured
and bind moxui to `127.0.0.1:8080`. **Then make sure the proxy enforces
HTTPS — moxui's plaintext mode is dev-only and emits a warning log on
startup.**

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `Failed to build JWT service: auth.jwt_private_key_pem_path is required` | JWT key path not set or file not readable | Generate keys (see above), check `chmod 0640` + `chown moxui:` |
| `cluster has insecure_skip_verify=true — TLS validation disabled` | Self-signed cert without `ca_cert_pem` | Either pin the CA cert or accept the warning for dev clusters |
| `Failed to open audit store: Permission denied` | `/var/lib/moxui` not writable by `moxui` user | `chown moxui:moxui /var/lib/moxui` |
| `binding plaintext listener` on startup | `server.tls` not set in config | Configure TLS (recommended) or accept dev mode |
