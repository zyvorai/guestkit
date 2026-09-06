# Using the Dashboard

GuestKit ships **CLI**, **TUI**, and an open-source **web Image Vault** (`deploy/ui` / GHCR `zyvor-ui`) over the same zyvor-api + guestkit-worker engine.

## Surfaces

| Surface | How to open |
|---------|-------------|
| **CLI** | `guestkit <cmd>` / `guestctl <cmd>` |
| **TUI** | `guestctl tui IMAGE` (aliases: `guestkit tui`, `guestkit ui`) |
| **Interactive REPL** | `guestkit interactive IMAGE` |
| **File explorer (CLI)** | `guestkit explore IMAGE [/path]` |
| **Web Image Vault** | GHCR compose → `http://<host>:8088` · or `./scripts/deploy-ui-remote.sh` (HTTPS lab) |

## Web Image Vault (OSS zyvor-ui)

Import a disk into the vault, then run the assure loop from the browser. The detail workspace mirrors TUI inventory + Assurance panes.

### Ops (Live API)

| Button | API | Notes |
|--------|-----|--------|
| **Inspect** | `POST /api/v1/vms/:id/inspect` | Fills Summary / Packages / Services / Users / Network / Storage / Kernel / Security |
| **Doctor** | `POST /api/v1/vms/:id/doctor` | Boot score on **Assurance** |
| **Migration plan** | `POST /api/v1/vms/:id/migration-plan` | Required changes + download JSON |
| **Passport** | `POST /api/v1/vms/:id/passport` | Download passport JSON |
| **Repair preview** | `POST /api/v1/vms/:id/repair-plan?dry_run=true` | Ops list (read-only) |
| **Apply repair** | same route `dry_run=false` | Gated confirm in Assurance pane — mutates the vault disk |
| **Profiles** | `POST /api/v1/vms/:id/profile` | Security/compliance findings + severity filter |
| **Provision YAML** | `POST /api/v1/vms/:id/provision` | KubeVirt manifests download |
| **Browse files** | `POST /api/v1/vms/:id/explore` | Read-only `ls` / `cat` (worker `guestkit.explore`) |

Offline mode: paste doctor JSON or load `demo-doctor.json` / `demo-inspect.json`.

### Lab HTTPS UI (no nginx)

```bash
./scripts/deploy-ui-remote.sh <host> <user> --port 27173 \
  --api-upstream http://127.0.0.1:8080
# → https://<host>:27173/
```

Compose / Helm still use GHCR `zyvor-ui` (nginx) on `:8088` — see [DOCKER.md](../guides/DOCKER.md#published-images-ghcr).

## TUI keys (Assurance)

| Key | Action |
|-----|--------|
| `d` | Doctor |
| `t` | Cycle migration target |
| `p` | Preview plan |
| `e` | Export plan |
| `:` | Command palette |
| `h` / `?` | Help |

## Browse vs act

Inspect / doctor / export / explore / profile are safe reads. **Repair apply**, **plan apply**, and **rescue** mutate disks — shut down the guest, dry-run first, keep a backup.

## Related

- [Getting Started](getting-started.md)
- [TUI](pages/interfaces/tui.md)
- [Common workflows](workflows.md)
- [OpenAPI](../../deploy/openapi/zyvor-vm-services.yaml)
