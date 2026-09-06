# Remote deployment

Deploy GuestKit to a Linux VM or bare-metal host over SSH, using the same workflow as [PacketWolf](../../../packetwolf/scripts/deploy-remote.sh).

> **Prefer prebuilt images?** The web stack is published to GHCR
> (`ghcr.io/zyvorai/{zyvor-ui,zyvor-api,guestkit-worker}`) — pull and run with
> `docker compose` or Helm instead of building from source. See
> [Docker → Published images](DOCKER.md#published-images-ghcr). This SSH workflow
> builds the **CLI binary** from source on the host.
>
> **Lab web UI only:** ship static `deploy/ui` with built-in HTTPS via
> `./scripts/deploy-ui-remote.sh <host> <user> --port 27173 --api-upstream http://127.0.0.1:8080`.
> Dashboard guide: [Using the Dashboard](../customer/using-the-dashboard.md).

## Quick start

```bash
# SSH key (recommended)
./scripts/deploy-remote.sh 10.0.0.5 root --key

# Or via Makefile
make deploy-remote H=10.0.0.5 U=root
```

## Profiles

| Profile | Flags | What it does |
|---------|-------|----------------|
| Full | *(default)* | rsync → qemu/nbd deps → rustup → `cargo build --release` → `/usr/local/bin` |
| Quick | `--quick` | rsync → build on remote (skip dep install) |
| Quick + local binary | `--quick --build-local` | build on Linux laptop, rsync binary only |
| Preflight | `--preflight-only` | SSH, disk, sudo checks |
| Verify | `--verify-only` | run `scripts/selftest.sh` on host |
| Uninstall | `--uninstall` | remove binary and `~/.deployments/guestkit` |

## Fleet rollout

```bash
# hosts.txt (chmod 600)
# 10.0.0.1 root --quick
# ops@10.0.0.2 --key --quick

./scripts/deploy-remote.sh --fleet hosts.txt
make deploy-remote-fleet FILE=hosts.txt
```

## Requirements

**Local:** `ssh`, `rsync`, optional `sshpass` for password auth.

**Remote:** Fedora/RHEL/CentOS (`dnf`/`yum`) or Debian/Ubuntu (`apt`). Non-root users need passwordless `sudo` for `modprobe` and `install`.

**Runtime deps (not legacy appliance tooling):** `qemu-img`, NBD module, `lvm2`/`parted` for some guests. GuestKit reads disks via its own Rust stack.

**Privileges:** Inspecting real VM disks usually requires root or membership in the `disk` group.

## Post-deploy

```bash
ssh root@10.0.0.5 'guestkit --version'
ssh root@10.0.0.5 'bash ~/.deployments/guestkit/scripts/selftest.sh'
```

## Web console access

The packaged install seeds a default administrator so you can sign in immediately:

| Field | Default |
|-------|---------|
| Username | `admin` |
| Password | `Admin@321` |
| API key | `Admin@321` (where applicable) |

> ⚠️ **Change these immediately after first login.** The defaults are seeded by
> `scripts/lib/package-auth-bootstrap.sh` into the product env file. Rotate the
> password (and API key / `JWT_SECRET`) before exposing the console to any
> untrusted network. SSO/SAML can be enabled from **Settings** to retire local login.
> OIDC providers must publish `jwks_uri` in discovery; ID tokens are verified with
> signature, issuer, audience (`client_id`), and expiry — unverified tokens are rejected.

## Logs

Set `GUESTKIT_DEPLOY_LOG` to capture a timestamped log under `~/.guestkit/`.
