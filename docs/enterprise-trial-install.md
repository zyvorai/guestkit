# GuestKit Enterprise — 30-day evaluation install

Try the **commercial control plane** (Command Center, Migration Factory, Passport Authority, Image Vault, SSO/RBAC) before you buy.

This is **not** the Apache-2.0 offline CLI in this repo (that CLI stays free).  
The evaluation package is a self-contained compiled binary attached to this repository — no source, no Node.js required.

**Download:** [Latest Enterprise trial release](https://github.com/zyvorai/guestkit/releases?q=enterprise-trial) · tag `v*-enterprise-trial`

---

## Requirements

| Requirement | Notes |
|---|---|
| OS / arch | Linux x86_64 (amd64) |
| Node.js | Not required — `bin/guestkit-enterprise-api` is a self-contained executable |
| Disk | ~100 MB |
| Docker | Optional — only for Keycloak SSO |

---

## Install

```bash
# Pick the tag from the release page, e.g. v1.0.0-enterprise-trial
TAG=v1.0.0-enterprise-trial
VER=1.0.0
ARCHIVE=guestkit-enterprise-${VER}-trial-linux-amd64.tar.gz

curl -LO "https://github.com/zyvorai/guestkit/releases/download/${TAG}/${ARCHIVE}"
curl -LO "https://github.com/zyvorai/guestkit/releases/download/${TAG}/${ARCHIVE}.sha256"
sha256sum -c "${ARCHIVE}.sha256"

tar xzf "${ARCHIVE}"
cd "guestkit-enterprise-${VER}-trial-linux-amd64"

# Accept Zyvor terms when prompted, or: export GUESTKIT_ZYVOR_ACCEPT=1
./install.sh
./test-package.sh
curl -s http://127.0.0.1:4000/health
```

Keep the bundled **`trial.token`** next to `install.sh`. Optional override:

```bash
export GUESTKIT_TRIAL_TOKEN="$(cat trial.token)"
```

### Login

```bash
curl -s -X POST http://127.0.0.1:4000/api/v1/auth/local \
  -H 'content-type: application/json' \
  -d '{"username":"demo","password":"demo"}'
```

Console: `cd console && python3 -m http.server 8081` → http://HOST:8081

---

## What's in the archive

| File | Purpose |
|---|---|
| `install.sh` | Start the API (no deps to install) |
| `trial.token` | Signed 30-day evaluation license |
| `INSTALL.md` / `QUICKSTART.txt` | In-archive copy of these steps |
| `AFTER-TRIAL.md` | What happens after expiry |
| `bin/guestkit-enterprise-api` | Compiled control-plane API (self-contained) |
| `console/` | Static console build (`expo export --platform web`) |
| `uninstall.sh` | Clean removal |

---

## Trial FAQ

- **How long?** 30 days from token issue — expiry is inside the signed JWT, not a local clock.
- **What happens after 30 days?** The API refuses to start and points you to **sales@zyvor.dev**.
- **Extension / full license?** Email **sales@zyvor.dev** — fastest path either way.
- **Open-source doctor?** Unaffected — keep using [this repo](https://github.com/zyvorai/guestkit) under Apache-2.0.

---

## Buy / demo

- [Book a demo](https://zyvor.dev/contact?intent=demo)
- [Product](https://zyvor.dev/guestkit) · [Pricing](https://zyvor.dev/pricing)
- [Open source vs Enterprise](ce-vs-enterprise.md)
