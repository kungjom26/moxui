# 🦀 MoxUI — Modern Proxmox UI

> **Modern, secure Rust-based web UI for Proxmox VE**
> Deployed as a single container. Designed for multi-cluster operations.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.78%2B-orange.svg)](https://www.rust-lang.org/)
[![Status: Pre-alpha](https://img.shields.io/badge/Status-Pre--alpha-red.svg)]()

---

## 🎯 What is MoxUI?

MoxUI is a web interface for [Proxmox VE](https://www.proxmox.com/en/proxmox-ve) written in **Rust**, designed to be:

- ⚡ **Fast** — frontend < 500 KB, API p99 < 200ms (cached)
- 🛡️ **Secure** — JWT + 2FA (TOTP + WebAuthn) + OIDC + LDAP, TLS 1.3, CSP
- 🌐 **Multi-cluster** — manage multiple Proxmox clusters from one dashboard
- 📦 **Single container** — one `docker run` and you're up
- 🎨 **Modern UI** — responsive, dark/light theme, keyboard shortcuts

**Status:** Pre-alpha — design phase complete, implementation starting.

---

## 📚 Documentation

| Doc | Purpose |
|---|---|
| [PROPOSAL.md](./PROPOSAL.md) | Project proposal + architecture + decisions |
| [ROADMAP.md](./ROADMAP.md) | Day-by-day implementation plan (6 weeks) |
| [FUTURE_ROADMAP.md](./FUTURE_ROADMAP.md) | Features planned for v1.1 → v3.0+ |
| [docs/FEATURE_SCOPE.md](./docs/FEATURE_SCOPE.md) | All 168 features with tier + acceptance criteria |

---

## 🛠️ Tech Stack

| Layer | Tech |
|---|---|
| Backend | Rust 1.78+ · axum 0.7 · tokio · rusqlite · reqwest |
| Frontend | Alpine.js · Tailwind CSS · uPlot · noVNC |
| Auth | JWT (RS256) · bcrypt · TOTP · WebAuthn · OIDC · LDAP |
| Deployment | Docker · docker-compose · Helm · Caddy |
| Observability | Prometheus · OpenTelemetry · tracing |

---

## 🚀 Quickstart (planned for v0.1.0-alpha)

```bash
# Once Phase 2 ships (Week 3)
docker run -d \
  --name moxui \
  -p 8080:8080 \
  -v moxui-data:/home/moxui/data \
  -v ./config.yaml:/etc/moxui/config.yaml:ro \
  ghcr.io/kungjom26/moxui:latest
```

Access `http://localhost:8080` → login → connect first Proxmox cluster.

---

## 📦 Features (v1.0.0 MVP)

**54 MUST features** including:

- 🖥️ VM management (start/stop/reboot/delete, console via noVNC, stats charts)
- 📊 Multi-cluster dashboard
- 🔐 Auth: local + TOTP + WebAuthn + OIDC + LDAP
- 🛡️ RBAC (admin/operator/viewer) + per-cluster permissions
- 📜 Audit log (every action captured)
- 🐳 Single-container deploy with TLS via Caddy
- 📈 Prometheus metrics + structured logging

See [docs/FEATURE_SCOPE.md](./docs/FEATURE_SCOPE.md) for full list.

---

## 🗓️ Roadmap

| Phase | When | Deliverable |
|---|---|---|
| **Phase 0** | Week 1 | Foundation + ProxmoxClient + cache |
| **Phase 1** | Week 2 | Core API + dashboard UI + console |
| **Phase 2** | Week 3 | **MVP v0.1.0-alpha** — auth + audit + Docker |
| **Phase 3** | Week 4-5 | Multi-cluster + OIDC + metrics |
| **Phase 4** | Week 6 | **v1.0.0** production release |
| **v1.1-v3.0** | Q4 2026 → Q4 2027 | [FUTURE_ROADMAP.md](./FUTURE_ROADMAP.md) |

---

## 🤝 Contributing

Currently in design phase — implementation starts after proposal approval.

When ready:
1. Read [PROPOSAL.md](./PROPOSAL.md) + [ROADMAP.md](./ROADMAP.md)
2. Pick a task from the current phase
3. Open a PR with tests + docs

---

## 📜 License

MIT — see [LICENSE](./LICENSE)

Copyright (c) 2026 kungjom26

---

## 🔗 Links

- **Repository:** https://github.com/kungjom26/moxui
- **Container:** ghcr.io/kungjom26/moxui
- **Issues:** https://github.com/kungjom26/moxui/issues
- **Proxmox API docs:** https://pve.proxmox.com/pve-docs/api-viewer/

---

> Built with 🦀 Rust and 🦐 by กุ้งจ่อม (Hermes Agent)