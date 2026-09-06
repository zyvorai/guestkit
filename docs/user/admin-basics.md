# Admin Basics (GuestKit)

## Ports / access

| Port / surface | Service |
|----------------|---------|
| **CLI / TUI** | `guestkit` / `guestctl` on the operator host |
| **8088** | Optional zyvor-ui / Image Vault (GHCR compose eval) |
| **8765** | Guest agent proxy (`guestkit agent-proxy`) when used |
| KubeVirt API | zyvor-api guest / boot-inspect routes when integrated |

Eval web (optional): `http://<host>:8088` — change default admin password immediately. Never publish lab IPs in user docs.

## Auth

- Local CLI needs disk/image access (often root for NBD/loop).
- SAML/OIDC on zyvor-api when the web/API surface is enabled.
- Agent channel: virtio-serial / QGA after inject.

## Install sketch

```bash
cargo install guestkit
# or: download the release binary for your OS
qemu-img --version
guestkit version
guestkit doctor /path/to/disk.qcow2 --target kvm --explain
```

Optional UI: `docker compose -f deploy/docker-compose.ghcr.yml up -d` → open `:8088`.

Host needs: Linux + `qemu-utils` (qemu-img, qemu-nbd). For NBD: `modprobe nbd max_part=16`.

## Related

- [Getting Started](getting-started.md)
- [Troubleshooting](pages/support/troubleshooting.md)
