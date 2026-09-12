# GuestKit

<p align="center">
  <strong>Offline VM intelligence. Migration assurance you can prove.</strong><br/>
  Score boot readiness <em>before</em> power-on · repair disks offline · certify cutover with a Passport
</p>

<p align="center">
  <a href="https://github.com/zyvorai/guestkit/actions/workflows/ci.yml"><img src="https://github.com/zyvorai/guestkit/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://crates.io/crates/guestkit"><img src="https://img.shields.io/crates/v/guestkit.svg" alt="crates.io"></a>
  <a href="https://pypi.org/project/hypersdk-guestkit/"><img src="https://img.shields.io/pypi/v/hypersdk-guestkit.svg" alt="PyPI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="Apache-2.0"></a>
  <a href="https://github.com/orgs/hypersdk/packages"><img src="https://img.shields.io/badge/GHCR-hypersdk-black?logo=github" alt="GHCR"></a>
</p>

<p align="center">
  <a href="https://zyvor.dev/guestkit?utm_source=github&utm_medium=guestkit"><b>Product</b></a> ·
  <a href="#see-it-in-action"><b>Demos</b></a> ·
  <a href="#who-does-what-users"><b>Suite path</b></a> ·
  <a href="#quick-start"><b>Quick start</b></a> ·
  <a href="#h2kvm-integration"><b>h2kvm</b></a> ·
  <a href="https://github.com/zyvorai/fluxvm"><b>FluxVM</b></a> ·
  <a href="https://github.com/zyvorai/guestkit/wiki"><b>Wiki</b></a> ·
  <a href="docs/ce-vs-enterprise.md"><b>Open source vs Enterprise</b></a> ·
  <a href="docs/enterprise-trial-install.md"><b>30-day Enterprise trial</b></a> ·
  <a href="https://zyvor.dev/contact?utm_source=github&utm_medium=guestkit&intent=demo"><b>Book a demo</b></a>
</p>

---

## The cutover problem — solved offline

Every hypervisor exit fails the same way: you discover the disk was broken **at 2am**, in the cutover window, after power-on.

GuestKit reads the disk **while the guest is off**, scores first-boot probability 0–100, and emits a reviewable fix plan — no appliance daemon, no “just try it and hope.”

```text
  disk.qcow2 / .vmdk / .vhdx / .vhd / .vdi / .raw
                    │
                    ▼
         ┌──────────────────────┐
         │  Pure-Rust engine    │──►  doctor 0–100 + blockers
         │  NBD / loop mount    │──►  migrate-plan YAML
         └──────────────────────┘──►  Passport · repair · CI gate
                    │                 guestkit-qemu (assured launch)
      CLI · TUI · QEMU · Python · Web · Agent · GitHub Action
```

| | |
|---|---|
| **70+** commands | **6** disk formats |
| ****0** appliance daemons | **8** migration targets |
| **Apache-2.0** | Used in CI, labs, and hypervisor-exit programs |

**Certify with [GuestKit](https://github.com/zyvorai/guestkit) → run & manage with [FluxVM](https://github.com/zyvorai/fluxvm) → convert & deploy with [h2kvm](https://github.com/zyvorai/h2kvm) → operate on [Zeus OS](https://zyvor.dev/zeus-os).**

---

## Who does what (users)

| You need… | Use |
|-----------|-----|
| Score / repair a disk **before** power-on | **This repo (GuestKit)** |
| Boot the qcow2, give it a network, SSH, TTL, pause/resume | **[FluxVM](https://github.com/zyvorai/fluxvm)** |
| Hypervisor → KVM convert + import | **[h2kvm](https://github.com/zyvorai/h2kvm)** |

GuestKit does **not** own production networking (TAP/bridge/netns/DHCP) or disposable
fleet lifecycle. That is FluxVM. Keep GuestKit focused on offline intelligence.

### End-to-end: certify → run → manage

```bash
# ── 1. Certify & repair (GuestKit) ─────────────────────────────
guestkit doctor disk.qcow2 --target kvm --explain
guestkit plan generate disk.qcow2 -p virtio-initramfs -o virtio.yaml
guestkit plan apply virtio.yaml --vm disk.qcow2 --yes   # as needed
guestkit gate --image disk.qcow2 --fail-below 80        # CI / cutover gate
guestkit passport emit disk.qcow2 --target kvm -o passport.json

# ── 2. Run & manage (FluxVM) ─────────────────────────────────
# Point FluxVM at the same (or repaired) qcow2 — see FluxVM README.
# Overlay keeps the base disk untouched; pick a network mode:
#
#   user     — lab SSH via hostfwd (simplest)
#   tap      — join existing bridge (LAN DHCP)
#   tap+netns— known guest IP + NAT (isolated)
#
#   fluxvm create --spec my-vm.json
#   fluxvm get <id>          # status + guest_ip when netns
#   fluxvm exec <id> -- uptime
#   fluxvm delete <id>
```

Docs: [VM lifecycle / suite split](docs/features/vm-runtime.md) ·
[FluxVM](https://github.com/zyvorai/fluxvm) ·
[Passport handoff](docs/user-guides/handoff-quarantine.md)

### Libvirt / virsh → suite map

| Old habit | Replacement |
|-----------|-------------|
| `virsh define` / `start` / `destroy` (host-local QEMU) | **[FluxVM](https://github.com/zyvorai/fluxvm)** `create` / `get` / `delete` |
| libvirt NAT / bridge DHCP / guest IP | FluxVM `user` / `tap`+bridge / `tap`+`netns` (`guest_ip`) |
| `virsh qemu-agent-command` | `guestkit qga` (or `fluxvm exec` with vsock agent) |
| “Will it boot?” by `virsh start` | `guestkit doctor` / `passport` / `gate` **before** FluxVM create |
| KubeVirt / OpenShift domains | `virtctl` / Machina (unchanged) |

Full map: [virsh-to-guestkit.md](docs/user-guides/virsh-to-guestkit.md).

---

## See it in action

<table>
<tr>
<td width="50%" align="center">
<a href="https://www.youtube.com/watch?v=lLEBQoFceIs">
<img src="https://i.ytimg.com/vi/lLEBQoFceIs/maxresdefault.jpg" alt="GuestKit CLI and TUI demo" width="100%">
<br><b>▶ CLI &amp; TUI</b>
</a>
<br><sub>Offline VM intelligence, explained</sub>
</td>
<td width="50%" align="center">
<a href="https://www.youtube.com/watch?v=usQX2rQIFM8">
<img src="https://i.ytimg.com/vi/usQX2rQIFM8/maxresdefault.jpg" alt="GuestKit web dashboard overview" width="100%">
<br><b>▶ Web Dashboard — Overview</b>
</a>
<br><sub>Server Image Vault, live KubeVirt cluster</sub>
</td>
</tr>
<tr>
<td width="50%" align="center">
<a href="https://www.youtube.com/watch?v=icTLVko588A">
<img src="https://i.ytimg.com/vi/icTLVko588A/maxresdefault.jpg" alt="GuestKit web dashboard deep dive" width="100%">
<br><b>▶ Web Dashboard — Deep Dive</b>
</a>
<br><sub>Sources, live cluster, one-click intelligence</sub>
</td>
<td width="50%" align="center">
<a href="https://www.youtube.com/watch?v=LYoqOye3P3I">
<img src="docs/img/machina-guestkit-demo-thumb.jpg" alt="Machina × GuestKit live guest-agent UX" width="100%">
<br><b>▶ Machina × GuestKit</b>
</a>
<br><sub>Live Linux guest agent — health, TRIM, netplan, services</sub>
</td>
</tr>
</table>

Recorded live against real deployments — no staged screenshots.

---

## Why teams switch

| Before GuestKit | With GuestKit |
|-----------------|---------------|
| “Will it boot?” answered at power-on | Offline **doctor** score + root-cause chain |
| guestkit scripts and tribal knowledge | Structured plans, JSON/YAML, CI gates |
| Surprises on cutover weekend | Hypervisor-aware **migrate-plan** + day-0 packs |
| No audit trail MTV / virt-v2v can skip | Signed **Cutover Passport** |
| Fleet drift invisible until outage | `fleet analyze` / `watch`, forensic diff, policy-as-code |
| Migration order guessed by hand | `fleet wave-plan` — dependency-aware waves |
| Deep inspect needs a running guest | Carbon **TUI** + in-guest agent over QGA |
| Assured first boot still means hand-built QEMU argv | **`guestkit-qemu`** plans/runs from the same evidence gate |
| Live guest ops still mean `virsh qemu-agent-command` | **`guestkit qga`** / `agent-call` speak the QGA socket directly |

---

## 60-second quick start

<a id="quick-start"></a>

```bash
cargo install guestkit          # guestkit + guestctl + guestkit-qemu

guestkit doctor vm.qcow2 --target proxmox --explain
guestkit migrate-plan vm.vmdk --target kvm --export plan.yaml
guestkit passport emit vm.qcow2 --target kvm -o passport.json
guestctl tui vm.qcow2           # Assurance · preview · export
guestkit-qemu plan vm.qcow2 --json   # assurance → QEMU definition

# Shrink an oversized-but-mostly-empty disk to its real footprint before import
guestkit shrink disk.qcow2 --dry-run                     # report only
guestkit shrink disk.qcow2 --min-ratio 3 --headroom-pct 20
```

**CI gate** — same score, no CLI install step:

```yaml
- uses: zyvorai/guestkit@v1
  with:
    disk: vm.qcow2
    target: kvm
    fail-below: '80'
```

Targets: `kvm` · `proxmox` · `qemu` · `kubevirt` · `aws` · `azure` · `gcp` · `hyperv`

Host needs: Linux with `qemu-img`, `losetup`, and `qemu-nbd` (mount/repair may need root).

### Python (v1.1.0+)

Same assurance engine as the CLI — on **[PyPI](https://pypi.org/project/hypersdk-guestkit/1.1.0/)** and used by **h2kvm** offline fixer:

```bash
pip install "hypersdk-guestkit>=1.1.0"
```

```python
import guestkit

guestkit.run_doctor("vm.qcow2", target="kvm", explain=True)
guestkit.run_migrate_repair("vm.qcow2", target="kvm", apply=False)  # dry-run
guestkit.run_migrate_repair("vm.qcow2", target="kvm", apply=True)   # apply fixes
```

See [python-bindings.md](docs/user-guides/python-bindings.md) and [examples/python/assurance_doctor.py](examples/python/assurance_doctor.py).

| You want… | Go here |
|-----------|---------|
| First hour | [Getting started](docs/user-guides/getting-started.md) |
| Python assurance APIs | [python-bindings.md](docs/user-guides/python-bindings.md) |
| **Assured QEMU launch** | [qemu-runtime.md](docs/features/qemu-runtime.md) |
| **h2kvm pipeline** | [hyper2kvm-integration.md](docs/features/hyper2kvm-integration.md) |
| Remote SSH deploy | [DEPLOY-REMOTE.md](docs/guides/DEPLOY-REMOTE.md) |
| Cheat sheet | [Quick reference](docs/user-guides/quick-reference.md) |
| Full feature map | [User feature guide](docs/guestkit-user-feature-guide.md) |
| Open source vs Enterprise | [ce-vs-enterprise.md](docs/ce-vs-enterprise.md) |

---

## h2kvm integration

GuestKit provides **offline disk intelligence**; [h2kvm](https://github.com/zyvorai/h2kvm) provides **hypervisor-to-KVM conversion and deploy**.

```text
  guestkit doctor / migrate-plan     ← pre-flight score + fix plan
              │
              ▼
  h2kvmctl local --backend guestkit  ← convert + run_migrate_repair
              │
              ▼
  libvirt · KubeVirt · OpenStack
```

```bash
# GuestKit from PyPI; h2kvm from GitHub Release
pip install "hypersdk-guestkit>=1.1.0"
pip install https://github.com/zyvorai/h2kvm/releases/download/v1.1.0/h2kvm-1.1.0-py3-none-any.whl

# Pre-flight
guestkit doctor source.vmdk --target kvm --explain

# Convert + offline repair
h2kvmctl local --vmdk source.vmdk --to-output out.qcow2 --backend guestkit
```

Deploy both to a lab host:

```bash
GUESTKIT_ZYVOR_ACCEPT=1 ./scripts/deploy-remote.sh HOST user --quick --key   # GuestKit CLI
cd /path/to/h2kvm && ./scripts/deploy-remote.sh HOST user --keep-sources      # h2kvm
```

Full guide: **[hyper2kvm-integration.md](docs/features/hyper2kvm-integration.md)** · **[h2kvm README](https://github.com/zyvorai/h2kvm#readme)**

---

## What you can do

### Assure · plan · certify · launch

```bash
guestkit doctor vm.qcow2 --target proxmox --explain
guestkit migrate-plan vm.vmdk --target proxmox --export plan.yaml
guestkit passport emit vm.qcow2 --target kvm -o passport.json
guestkit passport verify passport.json --fail-below 80
guestkit-qemu run vm.qcow2 --min-boot-score 80 --qmp-socket /run/guestkit/vm.qmp
```

### Repair offline (no boot required)

```bash
guestkit plan generate disk.qcow2 -p linux-ssh --user ubuntu --key-file ~/.ssh/id_ed25519.pub
guestkit rescue disk.qcow2 -o enable-ssh
guestkit rescue disk.qcow2 -o fix-grub --force
guestkit rescue win.qcow2 -o reset-password --user Administrator --password '…'
guestkit plan apply plan.yaml --vm disk.qcow2 --yes     # backups + rollback
```

### Local VM lifecycle, disk tools, and cutover

```bash
guestkit vm define demo disk.qcow2 --memory-mb 4096 --vcpus 2
guestkit vm start demo && guestkit vm status demo
guestkit img check disk.qcow2 --repair
guestkit domain-disks /etc/libvirt/qemu/web01.xml
guestkit firstboot win.qcow2 --hostname web01 --run 'echo hi'
guestkit gate --image disk.qcow2 --fail-below 80 --rego policies/cutover.rego
guestkit sbom-diff before.spdx.json after.spdx.json --fail-on-drift
virtctl-guestkit guestfs -n ns pvc
```

- **`guestkit vm`** — local QEMU lifecycle (define/start/pause/resume/destroy) for a lab or single box; production run/network/TTL is FluxVM's job — [features/vm-runtime.md](docs/features/vm-runtime.md)
- **`guestkit img` / `domain-disks` / `firstboot`** — qemu-img wrapper, libvirt/YAML domain disk parsing, virtio-win plan, first-boot gate — [user-guides/img-firstboot.md](docs/user-guides/img-firstboot.md)
- **Cutover bundle** — `gate` + SELinux/sysprep/BitLocker prep + cloud cutover profiles/Rego policy checks — [user-guides/cutover-bundle.md](docs/user-guides/cutover-bundle.md), [user-guides/cutover-prep.md](docs/user-guides/cutover-prep.md)
- **Passport handoff / fleet quarantine** — hand a passport to an h2kvmctl job, quarantine a fleet — [user-guides/handoff-quarantine.md](docs/user-guides/handoff-quarantine.md)
- **Rescue dry-run Action + `sbom-diff`** — forensic-diff SBOM attach, CI extras — [devops/10-rescue-sbom-ci.md](docs/devops/10-rescue-sbom-ci.md)
- **`virtctl-guestkit guestfs`** — drop-in for `virtctl guestfs` on a PVC, backed by GuestKit not libguestfs — [features/virtctl-guestkit.md](docs/features/virtctl-guestkit.md)

### Live control · platform · AI

- **In-guest agent** (Linux + Windows) over virtio-serial / QGA — inject offline, then `agent-proxy` / `agent-call`
- **`guestkit qga`** — drop-in for `virsh qemu-agent-command` (direct unix socket; no virsh by default) — [virsh-to-guestkit.md](docs/user-guides/virsh-to-guestkit.md)
- **Optional AI** (`--features ai`) — read-only tool-calling over the offline evidence snapshot; MCP server via `--features mcp`
- **KubeVirt** boot-inspect hooks and Guest Control Fabric
- **Web console** + worker on GHCR · Helm under `deploy/helm/zyvor`
- **Python:** `pip install hypersdk-guestkit` → `import guestkit` + `run_doctor` / `run_migrate_repair` (v1.1.0+)

---

## Run the free web stack (GHCR)

Public images under **`ghcr.io/zyvorai`** — no `docker login` required.

| Image | Role |
|-------|------|
| `ghcr.io/zyvorai/zyvor-ui` | Web console — Image Vault, KubeVirt cluster |
| `ghcr.io/zyvorai/zyvor-api` | API |
| `ghcr.io/zyvorai/guestkit-worker` | Disk-inspection worker |

```bash
docker compose -f deploy/docker-compose.ghcr.yml pull
docker compose -f deploy/docker-compose.ghcr.yml up -d
open http://localhost:8088
```

> **Eval only** — unauthenticated stack. Do not expose beyond localhost.  
> Production: `deploy/docker-compose.prod.example.yml` · [Docker guide](docs/guides/DOCKER.md) · [Helm](deploy/helm/zyvor)

---

## Open source vs Enterprise

<table>
<tr>
<td width="50%" valign="top">

### Open source — free under Apache-2.0
**This repo** · personal, lab, and commercial production

- Full offline **doctor**, migrate-plan, repair, fleet, policy  
- CLI · TUI · Python · self-hosted web/workers  
- GitHub Action Passport gate  
- Free `zyvor-ui` Image Vault dock  
- Best for labs, CI, small fleets, and production without Enterprise extras  

</td>
<td width="50%" valign="top">

### Enterprise — buy for programs
**[zyvor.dev/guestkit](https://zyvor.dev/guestkit?utm_source=github&utm_medium=guestkit)**

- Same engine — **not** a locked doctor  
- **Command Center** · Portfolio · Assurance  
- **Image Vault** (inspect/doctor/repair/migrate-plan, sources, batch, launch YAML, agent)  
- **Migration Factory** · **Passport Authority** (+ JSON download)  
- Dependencies · Policies · Compliance · **Reports** (JSON/CSV)  
- **Sites & Workers** · **KubeVirt** · Integrations · **Copilot** · Admin  
- OIDC / RBAC / audit · mobile console · command palette  
- SLA · air-gap · **hypervisor exit** workshops  
- Pipeline: HyperSDK → **h2kvm** → GuestKit → **Zeus OS** → PacketWolf  

</td>
</tr>
</table>

> **One failed first-boot weekend costs more than the license.**  
> Enterprise turns offline scores into shared, gated decisions your board can fund.

### 30-day Enterprise trial (binary)

Try the control plane before you buy — same packaging pattern as Veyron:

1. Download the **trial** asset from [GitHub Releases](https://github.com/zyvorai/guestkit/releases?q=enterprise-trial) (`guestkit-enterprise-*-trial-linux-amd64.tar.gz`)
2. Verify the `.sha256`, extract, run `./install.sh`
3. Keep bundled `trial.token` next to the install — after 30 days email **sales@zyvor.dev**

**[Full install instructions →](docs/enterprise-trial-install.md)**

**[Full feature matrix (every screen) →](docs/ce-vs-enterprise.md)** · **[What Zyvor sells →](docs/zyvor-enterprise.md)** · **[Book a demo](https://zyvor.dev/contact?utm_source=github&utm_medium=guestkit&intent=demo)** · **[Pricing](https://zyvor.dev/pricing?utm_source=github&utm_medium=guestkit)** · [sales@zyvor.dev](mailto:sales@zyvor.dev)

---

## Platform layout

```text
┌────────────────────────────────────────────────────────────┐
│  guestkit CLI · guestctl TUI · guestkit-qemu · Python · Web │
├────────────────────────────────────────────────────────────┤
│  Rust evidence · boot scoring · fix-plan · QEMU/VirtIO plan │
├────────────────────────────────────────────────────────────┤
│  JSON · YAML · HTML · PDF · Passport · CI exit codes       │
└────────────────────────────────────────────────────────────┘
```

| Layer | In this repo |
|-------|----------------|
| **Engine** | Pure-Rust parsers + evidence schema · NBD/loop (`src/`, `crates/`) |
| **CLI / TUI** | `guestkit` · `guestctl` — doctor, passport, fleet, rescue |
| **QEMU runtime** | `guestkit-qemu` — assured plan/run + QMP ([qemu-runtime.md](docs/features/qemu-runtime.md)) |
| **Agent / QGA** | Linux + Windows · `agent-inject` / `agent-proxy` / **`guestkit qga`** ([virsh-to-guestkit.md](docs/user-guides/virsh-to-guestkit.md)) |
| **Python** | [hypersdk-guestkit](https://pypi.org/project/hypersdk-guestkit/) — `run_doctor`, `run_migrate_repair` (v1.1.0+) |
| **h2kvm** | [hyper2kvm-integration.md](docs/features/hyper2kvm-integration.md) — convert/deploy partner |
| **FluxVM** | [zyvorai/fluxvm](https://github.com/zyvorai/fluxvm) — run/manage certified qcow2s (network, TTL) |
| **K8s** | KubeVirt hooks · `k8s/` |
| **Web / worker** | GHCR images · `deploy/` |

---

## Documentation

| Goal | Document |
|------|----------|
| Operator wiki | [zyvorai/guestkit/wiki](https://github.com/zyvorai/guestkit/wiki) |
| Docs home | [docs/README.md](docs/README.md) · [INDEX](docs/INDEX.md) |
| **DevOps runbooks** | [docs/devops](docs/devops/README.md) |
| Feature guide | [guestkit-user-feature-guide.md](docs/guestkit-user-feature-guide.md) |
| Docker / GHCR | [DOCKER.md](docs/guides/DOCKER.md#published-images-ghcr) |
| Remote deploy | [DEPLOY-REMOTE.md](docs/guides/DEPLOY-REMOTE.md) |
| **h2kvm integration** | [hyper2kvm-integration.md](docs/features/hyper2kvm-integration.md) |
| **QEMU / VirtIO runtime** | [qemu-runtime.md](docs/features/qemu-runtime.md) |
| **Dump virsh → GuestKit** | [virsh-to-guestkit.md](docs/user-guides/virsh-to-guestkit.md) |
| Architecture | [overview](docs/architecture/overview.md) |
| Changelog / roadmap | [CHANGELOG](docs/development/CHANGELOG.md) · [roadmap](docs/development/roadmap.md) |

→ [zyvor.dev/guestkit](https://zyvor.dev/guestkit) · [docs](https://zyvor.dev/docs?utm_source=github&utm_medium=guestkit) · [blog](https://zyvor.dev/blog?utm_source=github&utm_medium=guestkit)

---

## Development

```bash
cargo build --release
cargo test
```

See [CONTRIBUTING](docs/development/CONTRIBUTING.md) and CI under `.github/workflows/`. **`docs/` and this README are authoritative.**

---

## License

### Open source (Apache-2.0)

This repository is licensed under the [Apache License, Version 2.0](LICENSE).
You may use, modify, and run it for personal, lab, and commercial production
use at no charge, subject to Apache-2.0 (preserve notices / NOTICE where required).
See [NOTICE](NOTICE) and `docs/legal/` where applicable.

### Enterprise

Production support, SLAs, and Zyvor Enterprise products are licensed separately.
Contact [sales@zyvor.dev](mailto:sales@zyvor.dev) or see [zyvor.dev](https://zyvor.dev).
