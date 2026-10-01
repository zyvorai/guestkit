# GuestKit Guest Agent

> **Protocol 1.3** adds a rich heartbeat, rolling performance telemetry, a
> security choke point (expiry/nonce/idempotency), live migration
> assurance, cutover assist, and the `guestkitd`/`guestkitctl` naming —
> see **[GuestKit Agent — Protocol 1.3](guestkit-agent-protocol-1.3.md)**.

> **Not the same thing as the [AI copilot](../development/ai-guest-agent-roadmap.md).**
> This is `src/agent/` (`guestkitd`) — a live daemon running **inside the
> guest OS**. The AI copilot (`src/ai/`, `--features ai`) is a separate,
> optional LLM tool-calling loop that reads an offline `EvidenceSnapshot`
> and never touches a running guest. Same "agent" word, unrelated code.

GuestKit can run **inside** a Linux guest as an in-guest agent (similar to `qemu-guest-agent`), communicating with the host over virtio-serial while reusing the same evidence schema and fix-plan format as offline assurance.

On the **host**, prefer **`guestkit qga`** / **`guestkit agent-call`** against the QGA unix socket — do not call `virsh qemu-agent-command`. Cut-over map: [virsh-to-guestkit.md](../user-guides/virsh-to-guestkit.md).

## Architecture

```
Host (libvirt / KubeVirt virt-launcher)
          │  virtio-serial  org.qemu.guest_agent.0
          ▼
Guest VM: guestkit agent --channel virtio
```

Protocol: JSON-RPC 2.0 over 4-byte big-endian length-prefixed frames. QGA `{"execute":...}` commands are also supported on the same channel for KubeVirt compatibility.

## Build

```bash
# Default offline tool (unchanged)
cargo build --release

# In-guest agent artifact (static musl recommended for injection)
cargo build --release --features agent --no-default-features \
  --target x86_64-unknown-linux-musl
```

## Run in guest

```bash
guestkit agent --channel virtio
# Dev/test without VM:
guestkit agent --channel stdio
```

Systemd unit template: [`templates/agent/guestkit-agent.service`](../templates/agent/guestkit-agent.service)

## Host-side proxy

Connect to the libvirt channel unix socket and expose HTTP:

```bash
guestkit agent-proxy \
  --socket /var/lib/libvirt/qemu/channel/target/$VM/org.qemu.guest_agent.0 \
  --listen 127.0.0.1:8765
```

## Host-side one-shot RPC

For automation (e.g. VMRogue pod exec):

```bash
guestkit agent-call \
  --socket /var/run/kubevirt-private/.../org.qemu.guest_agent.0 \
  --method guestkit.getEvidence \
  --params '{}'
```

HTTP endpoints (via `agent-proxy`):

| Method | Path | RPC |
|--------|------|-----|
| GET | `/evidence` | `guestkit.getEvidence` |
| GET | `/doctor` | `guestkit.doctor` |
| GET | `/ping` | `guestkit.ping` |
| POST | `/fix-plan` | `guestkit.runFixPlan` |

## KubeVirt channel declaration

**KubeVirt 1.8+** (recommended): expose the QGA virtio-serial port via a disk serial — `devices.channels` is rejected on current KubeVirt releases.

```yaml
spec:
  domain:
    devices:
      disks:
      - name: guestagent
        disk:
          bus: virtio
        serial: org.qemu.guest_agent.0
  volumes:
  - name: guestagent
    emptyDisk:
      capacity: 1Gi
```

**KubeVirt &lt; 1.8** (legacy): virtio channel target:

```yaml
spec:
  domain:
    devices:
      channels:
      - name: org.qemu.guest_agent.0
        target:
          type: virtio
          name: org.qemu.guest_agent.0
```

Guest opens: `/dev/virtio-ports/org.qemu.guest_agent.0`

**Permissions:** the kernel creates the port `root:root 0600`, but `guestkit-agent.service` runs as `zyvor-agent`. The packages ship `60-zyvor-guest-agent.rules`, a udev rule that gives that group (mode 0660) the `org.qemu.guest_agent.0` and `com.zyvor.guestkit.0` ports, and reload/trigger it on install. Without it the agent starts but logs `failed to open virtio channel` and KubeVirt reports no guest agent.

**Linux install path:** cloud-init and `vmtools/install` place `zyvor-guest-agent` at `/usr/local/bin/zyvor-guest-agent`. Per-VM JSON-RPC via QGA guest-exec resolves the binary with `command -v` and falls back to `/usr/local/bin` then `/usr/bin`.

**Masquerade guests without cluster DNS:** prefer IP-only cloud-init (`curl` QGA deb + agent from MinIO) instead of `packages:` apt lines that require working DNS during first boot.

## Offline injection

During migration prep:

```bash
guestkit repair /path/to/disk.qcow2 --fix boot --inject-agent \
  --agent-binary ./target/x86_64-unknown-linux-musl/release/guestkit
```

A dedicated `agent-inject` subcommand installs the agent directly (Linux systemd
unit, or the Windows service via hivex):

```bash
# Linux (systemd unit -> /etc/systemd/system/zyvor-guest-agent.service)
guestkit agent-inject disk.qcow2 \
  --agent-binary ./target/x86_64-unknown-linux-musl/release/guestkit
```

### Windows offline install

`agent-inject --windows` provisions a Windows guest entirely offline through
guestkit's own hivex registry-write (requires building with
`--features registry-write`, which links `libhivex`):

```bash
guestkit agent-inject win.qcow2 --windows \
  --agent-binary ./target/x86_64-pc-windows-gnu/release/guestkitd.exe \
  --virtio-serial-driver /path/to/virtio-win/vioserial/w10/amd64
```

It:

1. Copies `guestkitd.exe` to `C:\guestkit\` and registers the `GuestKitAgent`
   service (auto-start, LocalSystem, `--service`) in the `SYSTEM` hive. The
   service answers the QGA virtio-serial channel exactly like the Linux agent.
2. Disables any stock `qemu-guest-agent` service found in the hive (`QEMU-GA`,
   `qemu-ga`, `QEMUGuestAgent` — set `Start=4`/disabled) so it can't contend
   with GuestKit for the same virtio-serial channel. GuestKit implements the
   QGA-compatible commands, so KubeVirt/libvirt stays connected either way;
   this is a no-op (and harmless) when no stock service is present.
3. Installs the **virtio-serial (`vioser`) driver** the QGA channel needs (only
   the boot-critical block driver ships with `--inject-virtio-win`): copies the
   driver files, adds them to `DevicePath`, and writes the driver service key +
   `CriticalDeviceDatabase` entries (parsed from the INF, including the KMDF
   binding).
4. On a **converted** image (e.g. VirtualBox eval → qcow2) the virtio-serial
   devnode is often cached as "no driver", so Windows never re-searches for one.
   guestkit deletes the stale `SYSTEM\...\Enum\PCI\VEN_1AF4&DEV_1043` device key
   so the PCI bus re-detects it as new next boot and runs a full INF install.

NTFS left dirty by a force-off is repaired (`ntfsfix`) so the offline writes
succeed. Verify from the host once booted:

```bash
# Preferred — GuestKit talks the QGA socket directly (no virsh)
guestkit qga --socket /var/lib/libvirt/qemu/channel/target/$VM/org.qemu.guest_agent.0 \
  --raw '{"execute":"guestkit-rpc","arguments":{"method":"guestkit.getVersion","params":{}}}'

# Same thing via GuestKit JSON-RPC
guestkit agent-call \
  --socket /var/lib/libvirt/qemu/channel/target/$VM/org.qemu.guest_agent.0 \
  --method guestkit.getVersion
# => {"protocol":"1.3","version":"0.3.14"}
```

## QGA compatibility

GuestKit implements common QGA commands including `guest-ping`, `guest-exec`, `guest-fsfreeze-freeze/thaw`, `guest-network-get-interfaces`, and `guest-get-host-name` for KubeVirt `AgentConnected`, snapshot freeze, and guest-exec subresources.

## RPC methods

- `guestkit.ping`
- `guestkit.getVersion`
- `guestkit.getCapabilities`
- `guestkit.getEvidence` (includes embedded `guest_health`)
- `guestkit.getStatus`
- `guestkit.getGuestHealth`
- `guestkit.getGuestInfo`
- `guestkit.getSystemdUnits`
- `guestkit.getSystemdUnit`
- `guestkit.getSystemdEvents`
- `guestkit.getProcesses`
- `guestkit.getFailedUnits`
- `guestkit.getBootAnalysis`
- `guestkit.getJournalSlice`
- `guestkit.getLoginState`
- `guestkit.getDnsState`
- `guestkit.getTimedateState`
- `guestkit.getSnapshotReadiness`
- `guestkit.doctor`
- `guestkit.migrateScore`
- `guestkit.getMetrics`
- `guestkit.freezeFilesystem` / `guestkit.thawFilesystem`
- `guestkit.restartUnit` (policy-gated; optional executor sidecar)
- `guestkit.executeRemediationPlan`
- `guestkit.collectSupportBundle`
- `guestkit.runFixPlan`
- `guestkit.runFixPlanRollback`

- QGA `execute` aliases include `guestkit-get-guest-health`, `guestkit-get-guest-info`, `guestkit-get-systemd-events`, `guestkit-get-processes`, and related commands.

## Local API

Read-only and policy-gated methods are available on `/var/run/zyvor/guest-agent.sock` (JSON-RPC framing).

Privileged remediation runs on `/var/run/zyvor/guest-agent-exec.sock` when `zyvor-guest-agent-exec.service` is enabled.

## Outbound Zeus push

Configure `/etc/zyvor/guest-agent.toml` with `zeus_url`, optional mTLS cert paths, and `interval_secs`. The agent registers and pushes `GuestHealth` + metrics to:

- `POST /api/v1/guest-agents/register`
- `POST /api/v1/guest-agents/{id}/heartbeat`
- `POST /api/v1/guest-agents/{id}/report`

## Worker jobs

When `guestkit-worker` runs with a live VM:

- `guestkit.agent.evidence` — fetch via agent-proxy HTTP
- `guestkit.agent.fix` — apply fix plan via agent-proxy

## Deep Linux intelligence (v1.2)

Collectors use **systemd D-Bus** (`org.freedesktop.systemd1.Manager` boot timestamps, full unit/service properties, live job/unit signals), **journald** via `journalctl` with cursor tracking and error pattern summaries, **process/cgroup** data from `/proc` (top CPU/memory, listening ports, PID→unit mapping), and **PSI pressure** from `/proc/pressure/*`.

`GuestHealth` now includes component scores (`boot`, `systemd`, `network`, `dns`, `storage`, `security`, `agent`), numeric `score`, `reasons`, and per-failed-unit `last_failure` text from journal correlation.

In-guest status (read-only):

```bash
zyvor-guest-agent status
# optional: --socket /var/run/zyvor/guest-agent.sock
```

Zeus API routes (via `zyvor-api`):

- `GET .../guest/health`
- `GET .../guest/journal?unit=&boot=current|previous`
- `GET .../guest/processes`
- `GET .../guest/systemd/units/{unit}`
- `GET .../guest/systemd/events`

Heartbeat/report push includes `recent_events` from the systemd D-Bus black-box recorder (stored in Redis). Push reports sync **VMGuestAgent** CR status with `guestHealth`, `healthScore`, `failedUnits`, and `systemdState` when namespace/vm are configured in `guest-agent.toml`.

Configure push identity:

```toml
zeus_url = "https://zeus.example.com"
namespace = "default"
vm_name = "my-vm"
bootstrap_token = "..."  # required when Zeus sets AGENT_BOOTSTRAP_TOKEN
```

**Guest remediation (Zeus UI):** restart failed units, collect a support bundle (`tar.zst` with evidence, health, semantic analysis, journal excerpts), and view per-unit journal slices from the Guest Intelligence card when a VM is selected.

When `GuestActionPolicy.spec.requireApproval` is true, remediation APIs return `pending_approval` with an `action_id`. Approve or reject via `POST /api/v1/guest-actions/{id}/approve` and `POST /api/v1/guest-actions/{id}/reject`, or use the Zeus fleet **Guest action approvals** panel and per-VM Guest Intelligence buttons. Audit history: `GET /api/v1/guest-actions/audit?limit=50`.

**mTLS bootstrap:** `POST /api/v1/guest-agents/bootstrap` issues client certificates when `AGENT_BOOTSTRAP_TOKEN` matches. Agents push heartbeats/reports on port 8443 when `AGENT_MTLS_PUBLIC_URL` is configured.

**Signed self-update:** `zyvor-guest-updater.timer` runs `zyvor-guest-agent --scheduled-update` daily (Linux). Windows guests register the same via `register-updater-task.ps1` (daily scheduled task). Updates require Ed25519-signed bundle manifests and SHA256-verified artifacts (`linux` tar or `windows` zip).

```bash
zyvor-guest-agent --check-update
zyvor-guest-agent --scheduled-update   # policy-gated auto-apply when enabled
```

**PacketWolf:** Zeus POSTs guest health to `PACKETWOLF_CORRELATION_URL` and runs fleet batch sweeps (`PACKETWOLF_FLEET_CORRELATE_URL`). UI: Guest Intelligence card + `POST /api/v1/packetwolf/fleet-correlate`.

**Hardening:** Linux agent runs as `zyvor-agent` with systemd sandbox; privileged remediation goes through `zyvor-guest-agent-exec`.

## Security

Same trust model as qemu-guest-agent: only the hypervisor host can connect to the virtio channel. Dangerous operations require `fix_apply` capability.

## Guest Control Fabric

For transport-independent control (QGA airgap install, capability contract, Agent Doctor, offline repair), see [guest-control-fabric.md](guest-control-fabric.md).
