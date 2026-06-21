# Installing moxui

MoxUI is distributed as a single static binary, an optional `.deb` package,
or as a source build. The release binary has no runtime dependencies
beyond `libc` and standard CA certificates (rustls links statically).

## Table of contents

1. [Prerequisites](#prerequisites)
2. [Option 1: Debian / Ubuntu (.deb)](#option-1-debian--ubuntu-deb)
3. [Option 2: Manual install (any distro)](#option-2-manual-install-any-distro)
4. [Option 3: From source (development)](#option-3-from-source-development)
5. [Generating JWT signing keys](#generating-jwt-signing-keys)
6. [Generating TLS certificates](#generating-tls-certificates)
7. [Hashing user passwords](#hashing-user-passwords)
8. [Verifying the install](#verifying-the-install)
9. [Uninstalling](#uninstalling)
10. [Unattended upgrades](#unattended-upgrades)
11. [Running behind a reverse proxy](#running-behind-a-reverse-proxy)
12. [Troubleshooting](#troubleshooting)
13. [Known issues (Day 8 candidates)](#known-issues-day-8-candidates)

## Prerequisites

For all install paths you will need:

- A Linux x86_64 host (other architectures will produce binaries via
  `cargo build --target` but are not packaged).
- A Proxmox VE cluster with a service account (we recommend a dedicated
  `moxui@pve` user with the `PVEVMAdmin` role or tighter).
- A reachable network path from moxui to the Proxmox API port (8006).
- Outbound HTTPS for `cargo` to fetch crates (build only).
- `openssl` (always) and `dpkg-deb` + `fakeroot` (only for `.deb` builds).

The release binary has **no runtime dependencies** beyond `libc` and the
system CA bundle (`/etc/ssl/certs/ca-certificates.crt` on Debian/Ubuntu,
`/etc/pki/tls/certs/ca-bundle.crt` on RHEL-family).

## Option 1: Debian / Ubuntu (.deb)

The `.deb` package installs the binary, the hardened systemd unit, a
sample config, and creates the `moxui` system user. It does **not**
auto-start the service — you must edit the config first.

```bash
# 1. Build the package locally
make package-deb
# → produces moxui_0.1.0_amd64.deb in the project root

# 2. Install (this creates the moxui user, sets up /var/lib/moxui, etc.)
sudo dpkg -i moxui_0.1.0_amd64.deb

# 3. Copy + edit the config. The package ships an EXAMPLE — values like
#    REPLACE_ME will make moxui refuse to start (fail-closed).
sudo cp /etc/moxui/config.yaml.example /etc/moxui/config.yaml
sudo chmod 0640 /etc/moxui/config.yaml
sudoedit /etc/moxui/config.yaml
#   - Set clusters[].url, username, password, realm for each Proxmox cluster
#   - Set clusters[].insecure_skip_verify: false (always, in production)
#   - Replace REPLACE_ME on every auth.users[].password
#     (or use password_hash — see "Hashing user passwords" below)

# 4. Generate JWT signing keys (separate from TLS keys)
sudo openssl genrsa -out /etc/moxui/jwt_priv.pem 4096
sudo openssl rsa -in /etc/moxui/jwt_priv.pem -pubout -out /etc/moxui/jwt_pub.pem
sudo chown moxui:moxui /etc/moxui/jwt_*.pem
sudo chmod 0640 /etc/moxui/jwt_*.pem

# 5. Generate TLS cert + key (self-signed for dev; use a real CA in prod)
sudo install -d -m 0750 -o moxui -g moxui /etc/moxui/tls
sudo openssl req -x509 -newkey rsa:2048 -nodes \
  -keyout /etc/moxui/tls/key.pem \
  -out /etc/moxui/tls/cert.pem \
  -days 36500 -subj "/CN=$(hostname -f)" \
  -addext "subjectAltName=DNS:$(hostname -f),IP:$(hostname -I | awk '{print $1}')"
sudo chown moxui:moxui /etc/moxui/tls/*
sudo chmod 0640 /etc/moxui/tls/*

# 6. Enable TLS in the config (uncomment the tls: block in server:)
sudo sed -i 's|^  # tls:|  tls:|; s|^  #   cert_pem_path:|  cert_pem_path:|' /etc/moxui/config.yaml
sudo sed -i 's|^  #   key_pem_path:|  key_pem_path:|' /etc/moxui/config.yaml
# (or just edit by hand with sudoedit)

# 7. Start the service
sudo systemctl daemon-reload
sudo systemctl enable --now moxui.service

# 8. Verify (see "Verifying the install" below)
sudo systemctl status moxui
curl -k https://localhost:8080/health
```

The postinst script creates the `moxui` user (if missing), sets up
`/var/lib/moxui` (state) and `/var/log/moxui` (logs), and reloads
systemd. It does **not** start or enable the service — that is always
manual after the operator has reviewed the config.

## Option 2: Manual install (any distro)

Use this path for RHEL, Fedora, Arch, or any non-Debian system. You
install the binary + unit, then create the moxui user yourself.

```bash
# 1. Build a release binary
make build-release
# → produces target/release/moxui (~11 MB stripped, LTO + abort-on-panic)

# 2. Install the binary + systemd unit (default PREFIX=/usr/local)
sudo make install
#   installs:
#     /usr/local/bin/moxui                          (binary, 0755)
#     /usr/local/lib/systemd/system/moxui.service   (unit, 0644)
#     /usr/local/share/doc/moxui/README.md          (docs, 0644)
#     /etc/moxui/config.yaml.example                (sample, 0640)
#   does NOT create the moxui user or /var/lib/moxui — do it now:

# 3. Create the dedicated moxui user
sudo useradd --system --home /var/lib/moxui --shell /usr/sbin/nologin moxui

# 4. Create state + log dirs (moxui writes audit log + DB here)
sudo install -d -m 0750 -o moxui -g moxui /var/lib/moxui /var/log/moxui

# 5. Create the config dir (the unit's ConfigurationDirectory hint
#    creates /etc/moxui, but unit-installed dirs depend on systemd version)
sudo install -d -m 0750 /etc/moxui
sudo install -d -m 0750 -o moxui -g moxui /etc/moxui/tls

# 6. Edit the config (same as Option 1 step 3)
sudo cp /etc/moxui/config.yaml.example /etc/moxui/config.yaml
sudo chmod 0640 /etc/moxui/config.yaml
sudoedit /etc/moxui/config.yaml

# 7. Generate JWT keys + TLS certs (same as Option 1 steps 4 + 5)
# ... (see those steps above)

# 8. Enable + start
sudo systemctl daemon-reload
sudo systemctl enable --now moxui.service

# 9. Verify
sudo systemctl status moxui
curl -k https://localhost:8080/health
```

To uninstall (does not remove state):

```bash
sudo make uninstall
sudo systemctl disable --now moxui.service
sudo rm -rf /var/lib/moxui /var/log/moxui  # only if you want a full wipe
```

## Option 3: From source (development)

For a quick local run during development. Runs as your user, uses dev
keys, and listens on 8080 without TLS (plaintext HTTP — dev only).

```bash
# 1. Generate dev JWT keys (separate from production keys)
mkdir -p .dev-keys
openssl genrsa -out .dev-keys/jwt_priv.pem 2048
openssl rsa -in .dev-keys/jwt_priv.pem -pubout -out .dev-keys/jwt_pub.pem

# 2. Create a dev config. The example file uses REPLACE_ME everywhere —
#    you must replace those values before moxui will start.
cp contrib/moxui.yaml.example .dev/config.yaml
$EDITOR .dev/config.yaml
#   At minimum, set clusters[].password and auth.users[].password
#   (the rest can stay as default for a dev cluster).

# 3. Run (auto-reload is NOT enabled — restart manually after edits)
MOXUI_CONFIG=$(pwd)/.dev/config.yaml cargo run --release
```

Useful environment variables for dev:

| Variable | Effect |
|---|---|
| `MOXUI_CONFIG=/path/to/config.yaml` | Override config file path (env wins over CLI) |
| `MOXUI_LOG_LEVEL=debug` | Set log level (trace, debug, info, warn, error) |
| `RUST_LOG=moxui=debug` | Standard `tracing` env (overrides MOXUI_LOG_LEVEL) |

CLI flags (from `moxui --help`):

```
moxui --config /etc/moxui/config.yaml  # path to config file
      --log-level info                  # trace|debug|info|warn|error
  -v, --verbose                         # short for --log-level debug
      --version                         # print version + exit
```

> **Note (Day 8 candidate):** the `--config` flag is currently parsed
> but not wired into `Config::load()`. The default path
> `/etc/moxui/config.yaml` is used, and `MOXUI_CONFIG` env var is the
> only way to override it. This is fine for production (default path
> matches) but limits development workflow flexibility. Tracked in
> [CHANGELOG known limitations](../CHANGELOG.md#known-limitations).

## Generating JWT signing keys

moxui uses RS256 JWTs (2048-bit RSA minimum, 4096-bit recommended for
production). You need both a private and public key.

```bash
# 4096-bit key (production)
openssl genrsa -out /etc/moxui/jwt_priv.pem 4096
openssl rsa -in /etc/moxui/jwt_priv.pem -pubout -out /etc/moxui/jwt_pub.pem

# 2048-bit key (faster, dev only — minimum accepted)
openssl genrsa -out /etc/moxui/jwt_priv.pem 2048
openssl rsa -in /etc/moxui/jwt_priv.pem -pubout -out /etc/moxui/jwt_pub.pem
```

Set ownership so moxui can read them:

```bash
sudo chown moxui:moxui /etc/moxui/jwt_*.pem
sudo chmod 0640 /etc/moxui/jwt_*.pem
```

moxui **refuses to start** if either key is missing or unreadable
(fail-closed). Rotation: replace both files, restart moxui — all
existing tokens become invalid (users must log in again).

## Generating TLS certificates

moxui enforces HTTPS in production. When `server.tls` is configured
in the config, the server listens with HTTPS only and refuses
plaintext HTTP. When omitted, the server logs a warning and serves
plaintext (development only).

### Self-signed (dev / test only)

```bash
sudo install -d -m 0750 -o moxui -g moxui /etc/moxui/tls
openssl req -x509 -newkey rsa:2048 -nodes \
  -keyout /etc/moxui/tls/key.pem \
  -out /etc/moxui/tls/cert.pem \
  -days 36500 \
  -subj "/CN=$(hostname -f)" \
  -addext "subjectAltName=DNS:$(hostname -f),DNS:localhost,IP:$(hostname -I | awk '{print $1}'),IP:127.0.0.1"
sudo chown moxui:moxui /etc/moxui/tls/*
sudo chmod 0640 /etc/moxui/tls/*
```

Verify the cert before pointing moxui at it:

```bash
openssl x509 -in /etc/moxui/tls/cert.pem -noout -subject -dates -ext subjectAltName
```

### Production (public CA)

Use Let's Encrypt via `certbot`, your internal CA, or a commercial CA.
The cert and key must be in PEM format. Point `server.tls.cert_pem_path`
and `server.tls.key_pem_path` at the files and restart the service.

For Let's Encrypt with the HTTP-01 challenge, you'll need port 80
reachable from the internet (or use DNS-01 instead). Place the cert
in `/etc/moxui/tls/cert.pem` and key in `/etc/moxui/tls/key.pem`.

## Hashing user passwords

`auth.users[].password` accepts plaintext (logged as a warning) or
`auth.users[].password_hash` (bcrypt-hashed, recommended for
production).

Generate a bcrypt hash:

```bash
# Use a Python one-liner (bcrypt is a stdlib dev dep in moxui's tree)
python3 -c "import bcrypt; print(bcrypt.hashpw(b'YOUR_PASSWORD', bcrypt.gensalt(rounds=12)).decode())"
```

Or use `htpasswd` (from `apache2-utils`):

```bash
htpasswd -bnBC 12 "" "YOUR_PASSWORD" | tr -d ':\n' | sed 's/^\$2y/\$2b/'
```

In the config, replace the `password:` line with:

```yaml
    - username: admin
      role: admin
      password_hash: "$2b$12$..."   # bcrypt hash
      disabled: false
```

moxui will validate the hash on startup. If both `password` and
`password_hash` are present, `password_hash` wins.

## Verifying the install

After `systemctl enable --now moxui.service`, check each of these:

```bash
# 1. Process running
sudo systemctl status moxui
# Expect: "active (running)" in green

# 2. Listening on the right port
ss -tlnp | grep moxui
# Expect: 0.0.0.0:8080 (or whatever server.bind is set to)

# 3. Health endpoint responds
curl -k https://localhost:8080/health
# Expect: JSON {"status":"ok", ...} with HTTP 200

# 4. Liveness + readiness (k8s-style)
curl -k https://localhost:8080/livez   # always 200 if the process is up
curl -k https://localhost:8080/readyz  # 200 only if at least one cluster pinged OK

# 5. Login works
TOKEN=$(curl -k -s -X POST https://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"YOUR_PASSWORD"}' \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['token'])")

# 6. Authenticated endpoint works
curl -k -H "Authorization: Bearer $TOKEN" https://localhost:8080/api/v1/auth/me
# Expect: {"username":"admin","role":"admin",...}

# 7. Audit log was opened (file exists + has the login event)
sudo ls -la /var/lib/moxui/moxui.db.audit
sudo sqlite3 /var/lib/moxui/moxui.db.audit 'SELECT event_type, user_id, status FROM events ORDER BY id DESC LIMIT 1;'
# Expect: most recent row is the login (status 200)
```

## Uninstalling

For the `.deb`:

```bash
sudo systemctl disable --now moxui.service
sudo dpkg --purge moxui
# This removes the binary, unit, sample config, and (with --purge) the
# /etc/moxui directory. State under /var/lib/moxui is preserved —
# wipe it manually if you want a full clean install later:
sudo rm -rf /var/lib/moxui /var/log/moxui /etc/moxui
```

For a manual install:

```bash
sudo systemctl disable --now moxui.service
sudo make uninstall
sudo userdel moxui && sudo groupdel moxui   # if you created the user
sudo rm -rf /var/lib/moxui /var/log/moxui /etc/moxui
```

## Unattended upgrades

The `.deb` package declares `/etc/moxui/config.yaml.example` as a
**conffile** — `apt upgrade` will ask before overwriting it. This is
the *template* file, not your real config. Your real
`/etc/moxui/config.yaml` is **not** a conffile and `apt upgrade` will
silently overwrite it with the new example on every package upgrade.

**Implication:** the current package design means `apt upgrade` will
clobber your config. To preserve it:

- **Recommended:** keep your config in version control (e.g. a private
  git repo) and apply it after each upgrade.
- **Workaround:** `chattr +i /etc/moxui/config.yaml` (immutable) — but
  you'll need to `chattr -i` before every manual edit.
- **Day 8 candidate:** change the postinst + conffiles to declare
  `/etc/moxui/config.yaml` (not the `.example`) as the conffile, and
  ship a `dpkg-maintscript-helper` to migrate the original
  `/etc/moxui/config.yaml` to a `.dpkg-dist` backup on first upgrade.

For now, **read the diff between your config and the new example after
every upgrade**:

```bash
diff -u /etc/moxui/config.yaml /etc/moxui/config.yaml.example
# Apply any new fields / renamed keys to your config, then restart:
sudo systemctl restart moxui.service
```

## Running behind a reverse proxy

If you want to terminate TLS at a reverse proxy (nginx, Caddy, Traefik)
instead of having moxui do it directly, leave `server.tls` unconfigured
and bind moxui to `127.0.0.1:8080`. Then **make sure the proxy
enforces HTTPS — moxui's plaintext mode is dev-only and emits a
warning log on every startup**.

Example Caddyfile:

```
moxui.example.com {
    reverse_proxy 127.0.0.1:8080
    # Caddy handles TLS automatically via Let's Encrypt
}
```

Example nginx:

```nginx
server {
    listen 443 ssl http2;
    server_name moxui.example.com;

    ssl_certificate     /etc/letsencrypt/live/moxui.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/moxui.example.com/privkey.pem;

    # Pass through to moxui
    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host              $host;
        proxy_set_header X-Real-IP         $remote_addr;
        proxy_set_header X-Forwarded-For   $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

moxui's security headers (HSTS, X-Frame-Options, etc.) are added in
its own middleware layer. If your proxy is in front, you can either let
moxui add them (HSTS will be sent on the plaintext hop, which most
browsers ignore) or add them at the proxy level. If you terminate TLS
at the proxy, set the HSTS header at the proxy too — it's only
honoured by browsers when received over HTTPS.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `Failed to build JWT service: auth.jwt_private_key_pem_path is required` | JWT key path not set or file not readable | Generate keys (see "Generating JWT signing keys"), check `chmod 0640` + `chown moxui:` |
| `Failed to open audit store: Permission denied` | `/var/lib/moxui` not writable by `moxui` user | `chown moxui:moxui /var/lib/moxui` |
| `binding plaintext listener` warning on startup | `server.tls` not set in config | Configure TLS (recommended) or accept dev mode |
| `cluster has insecure_skip_verify=true — TLS validation disabled` | Self-signed Proxmox cert without `ca_cert_pem` | Pin the CA cert in `clusters[].ca_cert_pem` (preferred) or set `insecure_skip_verify: true` (dev only) |
| `Failed to load config: ...` on startup | Bad YAML syntax or unknown field | Run `moxui --config /etc/moxui/config.yaml` (or `MOXUI_CONFIG=...`) directly to see the parse error; check against the example |
| 401 on every login attempt | User has `disabled: true`, or password hash mismatch | Check `auth.users[].disabled: false`; regenerate `password_hash` from the password you actually typed |
| 403 on a POST /api/v1/vms/.../start | Logged-in user has role `viewer` (read-only) | Use an `operator` or `admin` account; viewer cannot start/stop VMs |
| HSTS header missing from responses | moxui is running in plaintext mode | Configure `server.tls` in the config (the middleware only adds HSTS over HTTPS) |
| `address already in use` on startup | Port 8080 is taken by another process | Change `server.bind` in config, or stop the conflicting process |
| moxui unit fails with `status=203/EXEC` | Binary not at `/usr/bin/moxui` (manual install) | Either reinstall the `.deb`, or edit the unit to point at your install prefix (`/usr/local/bin/moxui`) and `systemctl daemon-reload` |
| TLS handshake fails with `unknown issuer` | Client doesn't trust the self-signed cert | Use `curl -k` for testing, or add the cert to the client's trust store |

## Known issues (Day 8 candidates)

These are tracked in [CHANGELOG known limitations](../CHANGELOG.md#known-limitations)
and will be addressed in the next phase:

1. **`--config` flag is dead** — `moxui --config /path/to/config.yaml`
   silently ignores the flag and uses `/etc/moxui/config.yaml`. The
   only way to override the config path is `MOXUI_CONFIG=/path/...`
   env var. Tracked as Day 8 bug.

2. **`/health`, `/auth/login`, `/auth/me` do not emit audit events** —
   only state-changing endpoints (VM start/stop/etc.) write to the
   audit log. Login attempts are NOT recorded.

3. **No audit log rotation** — `moxui.db.audit` grows without bound.
   Add `logrotate` config in `contrib/` and/or implement internal
   rotation based on file size.

4. **No JWT keypair generator** — use `openssl` (documented above).
   Future: add `moxui keygen jwt` subcommand.

5. **`apt upgrade` clobbers `/etc/moxui/config.yaml`** — see
   "Unattended upgrades" above. The current packaging only protects
   the `.example` template as a conffile. Will be fixed in Day 8.

6. **No HA / clustering** — single-node deployment only. Phase 3
   work.

7. **No WebSocket / live console** — VM console access requires
   WebSocket support (Phase 2).

For bug reports or to track these, see
https://github.com/kungjom26/moxui/issues.
