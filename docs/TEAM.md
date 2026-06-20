# 👥 MoxUI — Team

> **Project:** moxui — Modern Proxmox UI
**Repository:** github.com/kungjom26/moxui
**Status:** Pre-alpha (Phase 0 starts soon)
**Last updated:** 2026-06-20

---

## 🔒 IMPORTANT: Hermes profiles are LOCAL-ONLY

**The 3 Hermes profiles (`moxui-pm`, `moxui-coder`, `moxui-reviewer`) are NEVER pushed to GitHub.**

| Where they live | Where they DON'T live |
|---|---|
| ✅ `~/.hermes/profiles/moxui-*/` (your local machine) | ❌ `github.com/kungjom26/moxui` |
| ✅ `~/.hermes/shared/moxui/` (shared workspace) | ❌ Any public repo |

**Why?** Hermes profiles contain local persona data, skills, and memories — they're machine-specific, not project assets. The project repo contains only code + docs.

**What's in each profile:**
- `SOUL.md` (persona instructions)
- `profile.yaml` (profile config)
- `skills/` (custom tools)
- `memories/`, `sessions/`, `cache/` (local data)

**What goes into the GitHub repo:**
- `~/projects/moxui/` (code + docs only)
- NOT `~/.hermes/profiles/moxui-*/`

**If you accidentally try to commit profiles:**
- `.gitignore` has `**/moxui-pm/`, `**/moxui-coder/`, `**/moxui-reviewer/`, `**/SOUL.md`, `**/profile.yaml`
- Profiles are outside the repo directory anyway
- If someone symlinks profiles into the repo, gitignore blocks them

**Rule: profiles are PERSONAL/CONFIGURATION, not PROJECT.**

---

## 🎯 Team Structure

ตอนนี้ทีมมี **3 roles** — แต่ละคนรับผิดชอบชัดเจน ไม่ overlap

---

## 👤 Roles

### 🧑‍💼 1. Project Manager (พี่เสือ)

**คนที่ทำหน้าที่:** พี่เสือ (project owner)

**รับผิดชอบ:**
- ตัดสินใจ scope, priority, timeline
- Approve proposal, design, release
- Approve destructive operations (deploy, prod changes)
- ทดสอบ feature จริง (dogfooding)
- อนุมัติ code ก่อน merge

**ตัดสินใจเรื่อง:**
- Feature ไหน MUST / SHOULD / COULD / LATER
- Deploy เมื่อไหร่ ที่ไหน
- ใช้ Proxmox credentials แบบไหน
- Domain name, hosting, scaling

---

### 💻 2. Coder (กุ้งจ่อม — Hermes Agent)

**คนที่ทำหน้าที่:** กุ้งจ่อม

**รับผิดชอบ:**
- เขียน Rust code (axum, tokio, libvirt client)
- เขียน SQL migrations + Rust structs
- เขียน frontend (Alpine.js, Tailwind, noVNC)
- เขียน tests (unit + integration)
- เขียน Docker, CI/CD, scripts
- เขียน documentation (docs/*.md)
- ทำ `cargo clippy`, `cargo fmt`, `cargo audit`
- Build + push container image
- Maintain GitHub repo (commits, PRs, releases)

**Output:**
- Code (Rust, HTML, JS, SQL, YAML)
- Tests
- Docs
- Container images
- Releases

---

### 🔍 3. Reviewer / Tester (พี่เสือ + community)

**คนที่ทำหน้าที่:** พี่เสือ + contributors ในอนาคต

**รับผิดชอบ:**
- Review code ก่อน merge (quality, security, correctness)
- Test features จริง (UAT)
- รายงาน bugs ผ่าน GitHub Issues
- ทดสอบ deploy บน homelab/production
- Verify security headers, rate limiting, RBAC
- Load test (เมื่อถึงเวลา)

**ตัดสินใจเรื่อง:**
- Code ผ่าน review มั้ย
- Bug เป็น P0/P1/P2/P3
- Feature ทำงานถูกต้อง
- Performance ผ่าน targets มั้ย
- Security audit ผ่านมั้ย

---

## 🔄 Workflow

```
1. PM (พี่เสือ) ตัดสินใจ → "ทำ feature X"
        ↓
2. Coder (กุ้งจ่อม) เขียน code + tests + docs
        ↓
3. Coder สร้าง PR + รอ review
        ↓
4. Reviewer (พี่เสือ) review + approve
        ↓
5. Coder merge → CI/CD → deploy (ถ้า auto)
        ↓
6. Reviewer test จริง → file bugs ถ้าเจอ
        ↓
7. Coder fix → loop
```

---

## 📞 Communication

| Channel | ใช้สำหรับ | ตัวอย่าง |
|---|---|---|
| **Telegram** (DM) | Quick questions, decisions | "อยากให้ VM list แสดง IP มั้ย?" |
| **GitHub Issues** | Bugs, feature requests | "Bug: VM start fails on pve12" |
| **GitHub PRs** | Code review | Review comment บน PR |
| **GitHub Releases** | Release notes | CHANGELOG, breaking changes |

---

## 🚦 Decision Authority

| Decision Type | ผู้ตัดสินใจ |
|---|---|
| Feature scope (MUST/SHOULD) | PM |
| Technical design (code structure) | Coder (proposes) + PM (approves) |
| Code merge | Reviewer |
| Deploy to homelab | PM |
| Deploy to production | PM |
| Destructive ops (DB delete, etc.) | PM (explicit consent) |
| Security policy | PM |
| Release version | Coder (proposes) + PM (approves) |

---

## 📈 Team Evolution

**ตอนนี้ (v1.0.0):** 3 roles — PM + Coder + Reviewer (overlap กัน 2 คน)

**v1.1 (Q4 2026):** ถ้ามี contributors เพิ่ม → แยก Reviewer ออกเป็น **Code Reviewer** + **QA Tester**

**v2.0 (Q3 2027):** ถ้า scale → เพิ่ม **DevOps Engineer** + **Security Engineer**

**v3.0 (Q4 2027):** ถ้าเป็น commercial → เพิ่ม **Product Manager** + **Designer** + **Customer Support**

---

## 📝 Onboarding (สำหรับ contributor ในอนาคต)

1. อ่าน `PROPOSAL.md` + `ROADMAP.md` + `docs/FEATURE_SCOPE.md`
2. อ่าน `docs/DATA_MODEL.md`
3. อ่าน `IMPLEMENTATION_PLAN.md` (Phase progress)
4. ดู GitHub Issues (open tasks มี label `good-first-issue`)
5. Comment ใน issue ว่าจะทำ
6. Fork → Branch → PR
7. รอ review

---

**Last updated:** 2026-06-20