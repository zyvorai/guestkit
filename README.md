<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/social/guestkit-share-card-dark.png">
  <img src="docs/social/guestkit-share-card.png" alt="GuestKit — offline VM intelligence and migration assurance" width="820">
</picture>

# GuestKit

### Offline VM intelligence and migration assurance.

Score boot readiness before power-on, repair disks offline, and certify cutover with a Passport.

[![CI](https://img.shields.io/github/actions/workflow/status/zyvorai/guestkit/ci.yml?branch=main&style=flat-square&labelColor=1d1d1f&label=CI)](https://github.com/zyvorai/guestkit/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/guestkit?style=flat-square&color=0071e3&labelColor=1d1d1f)](https://crates.io/crates/guestkit)
[![PyPI](https://img.shields.io/pypi/v/zyvor-guestkit?style=flat-square&color=0071e3&labelColor=1d1d1f)](https://pypi.org/project/zyvor-guestkit/)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-0071e3?style=flat-square&labelColor=1d1d1f)](LICENSE)
[![GHCR](https://img.shields.io/badge/GHCR-zyvorai-0071e3?style=flat-square&labelColor=1d1d1f&logo=github)](https://github.com/orgs/zyvorai/packages)

[**Quick start**](#quick-start) · [**Gallery**](docs/gallery.md) · [**Docs**](https://zyvorai.github.io/guestkit/) · [**Product**](https://zyvor.dev/guestkit?utm_source=github&utm_medium=guestkit) · [**Wiki**](https://github.com/zyvorai/guestkit/wiki) · [**FluxVM**](https://github.com/zyvorai/fluxvm) · [**h2kvm**](https://github.com/zyvorai/h2kvm) · [**Book a demo**](https://zyvor.dev/contact?utm_source=github&utm_medium=guestkit&intent=demo)

</div>

---

## Know it will boot. Before you power it on.

GuestKit reads a VM disk **while the guest is off** — qcow2, VMDK, VHDX, VHD, VDI or raw — through its own pure-Rust engine (NBD or loop mount). There is no appliance daemon and no "power it on and see". It scores first-boot probability 0–100, explains the blockers, and writes a reviewable fix plan.

Fixes are never implicit. Repairs go through a plan you can read, applied with backups and rollback, and the same score drives the CI gate, the signed Cutover Passport, and assured QEMU launch.

<div align="center">

<img src="docs/img/ui-00-doctor-demo.png" alt="GuestKit web console — Assurance panel showing a boot score of 78 with ranked findings" width="820">

<sub>The bundled OSS web console rendering its offline demo data, not a live deployment. More in the <a href="docs/gallery.md">gallery</a>.</sub>

</div>

<a id="-see-it-in-action"></a>

<table>
<tr>
<td valign="top" width="33%">
<b>Boot-readiness scoring</b><br>
First-boot probability 0–100, the blockers explained, and a reviewable fix plan.<br>
<a href="docs/cutover-offline.md">The cutover problem</a>
</td>
<td valign="top" width="33%">
<b>Offline repair</b><br>
Repairs go through a plan you can read, applied with backups and rollback.<br>
<a href="docs/capabilities.md">What you can do</a>
</td>
<td valign="top" width="33%">
<b>Migration assurance</b><br>
The same score drives the CI gate, the signed Cutover Passport and assured QEMU launch.<br>
<a href="docs/quick-start.md">Quick start</a>
</td>
</tr>
<tr>
<td valign="top" width="33%">
<b>Engine and formats</b><br>
A pure-Rust engine over qcow2, VMDK, VHDX, VHD, VDI and raw, through NBD or loop mount.<br>
<a href="docs/platform-layout.md">Platform layout</a>
</td>
<td valign="top" width="33%">
<b>Suite hand-offs</b><br>
Certify with GuestKit, run and manage with FluxVM, convert and deploy with h2kvm.<br>
<a href="docs/who-does-what.md">Who does what</a>
</td>
<td valign="top" width="33%">
<b>CLI, TUI, web, CI</b><br>
CLI, TUI, QEMU, Python, web console, in-guest agent and a GitHub Action.<br>
<a href="docs/web-stack-ghcr.md">Run the web stack</a>
</td>
</tr>
</table>

<div align="center">

**70+** commands · **6** disk formats · **0** appliance daemons · **8** migration targets · **Apache-2.0**

</div>

---

<a id="quick-start"></a>

## 60-second quick start

```bash
# v1.2.4 GitHub Release — crates.io `guestkit` is still 0.3.2
curl -fsSL -O https://github.com/zyvorai/guestkit/releases/download/v1.2.4/guestkit-1.2.4-linux-amd64.tar.gz

guestkit doctor vm.qcow2 --target proxmox --explain
guestkit migrate-plan vm.vmdk --target kvm --export plan.yaml
guestkit passport emit vm.qcow2 --target kvm -o passport.json
guestctl tui vm.qcow2           # Assurance · preview · export
guestkit-qemu plan vm.qcow2 --json   # assurance → QEMU definition
```

**CI gate** — same score, no CLI install step:

```yaml
- uses: zyvorai/guestkit@v1
  with:
    disk: vm.qcow2
    target: kvm
    fail-below: '80'
```

Targets: `kvm` · `proxmox` · `qemu` · `kubevirt` · `aws` · `azure` · `gcp` · `hyperv`. Host needs: Linux with `qemu-img`, `losetup`, and `qemu-nbd` (mount/repair may need root).

Python (v1.1.0+), on **[PyPI](https://pypi.org/project/zyvor-guestkit/)**: `pip install zyvor-guestkit`, then `guestkit.run_doctor("vm.qcow2", target="kvm", explain=True)`. The full quick start, shrink and Python examples: [docs/quick-start.md](docs/quick-start.md).

---

## The cutover problem, solved offline

Every hypervisor exit fails the same way: you discover the disk was broken **at 2am**, in the cutover window, after power-on. GuestKit reads the disk **while the guest is off**, scores first-boot probability 0–100, and emits a reviewable fix plan.

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

<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/social/migration-1200x630-dark.png">
  <img src="docs/social/migration-1200x630.png" alt="VMware to KubeVirt, four tools one path: Transiva exports, h2kvm converts and deploys, GuestKit assures, Zorvia operates, each with Community and Enterprise tiers." width="820">
</picture>

</div>

**Certify with [GuestKit](https://github.com/zyvorai/guestkit) → run & manage with [FluxVM](https://github.com/zyvorai/fluxvm) → convert & deploy with [h2kvm](https://github.com/zyvorai/h2kvm) → operate on [Zeus OS](https://zyvor.dev/zeus-os) or the open-source [Zorvia](https://github.com/zyvorai/zorvia/blob/main/docs/leave-openshift.md).** [Who does what](docs/who-does-what.md) · [h2kvm integration](docs/h2kvm-at-a-glance.md)

<a id="why-teams-switch"></a>

## Why teams switch

| Before GuestKit | With GuestKit |
|-----------------|---------------|
| “Will it boot?” answered at power-on | Offline **doctor** score + root-cause chain |
| guestkit scripts and tribal knowledge | Structured plans, JSON/YAML, CI gates |
| Surprises on cutover weekend | Hypervisor-aware **migrate-plan** + day-0 packs |
| No audit trail MTV / virt-v2v can skip | Signed **Cutover Passport** |
| Migration order guessed by hand | `fleet wave-plan` — dependency-aware waves |

The [full comparison](docs/why-teams-switch.md) has four more rows.

---

## Open source, Enterprise

**Open source — free under Apache-2.0.** This repo · personal, lab, and commercial production. Full offline **doctor**, migrate-plan, repair, fleet, policy · CLI · TUI · Python · self-hosted web/workers.

**Enterprise — buy for programs.** Same engine — **not** a locked doctor. Command Center · Portfolio · Assurance · Migration Factory · Passport Authority · OIDC / RBAC / audit · SLA · air-gap · hypervisor exit workshops.

[Open source vs Enterprise](docs/oss-vs-enterprise.md) · [Full feature matrix](docs/ce-vs-enterprise.md) · [30-day Enterprise trial](docs/enterprise-trial-install.md) · [Pricing](https://zyvor.dev/pricing?utm_source=github&utm_medium=guestkit)

<a id="documentation"></a>

## Documentation

| Goal | Document |
|------|----------|
| Docs site | [zyvorai.github.io/guestkit](https://zyvorai.github.io/guestkit/) |
| Docs home | [docs/README.md](docs/README.md) · [INDEX](docs/INDEX.md) |
| DevOps runbooks | [docs/devops](docs/devops/README.md) |
| Feature guide | [guestkit-user-feature-guide.md](docs/guestkit-user-feature-guide.md) |
| h2kvm integration | [hyper2kvm-integration.md](docs/features/hyper2kvm-integration.md) |
| QEMU / VirtIO runtime | [qemu-runtime.md](docs/features/qemu-runtime.md) |
| Dump virsh → GuestKit | [virsh-to-guestkit.md](docs/user-guides/virsh-to-guestkit.md) |
| Architecture | [overview](docs/architecture/overview.md) |

The complete map, with the changelog and roadmap, is in [docs/documentation-map.md](docs/documentation-map.md).

## Go deeper

- <a id="gallery"></a>**Gallery:** console screenshots and demo videos — [docs/gallery.md](docs/gallery.md).
- <a id="the-cutover-problem--solved-offline"></a><a id="who-does-what-users"></a>**Who does what:** the suite split and the libvirt/virsh map — [docs/who-does-what.md](docs/who-does-what.md).
- <a id="h2kvm-integration"></a>**h2kvm integration:** [docs/h2kvm-at-a-glance.md](docs/h2kvm-at-a-glance.md).
- <a id="what-you-can-do"></a>**What you can do:** assure, plan, certify, repair, launch — [docs/capabilities.md](docs/capabilities.md).
- <a id="run-the-free-web-stack-ghcr"></a>**Free web stack (GHCR):** [docs/web-stack-ghcr.md](docs/web-stack-ghcr.md).
- <a id="open-source-vs-enterprise"></a>**Open source vs Enterprise:** [docs/oss-vs-enterprise.md](docs/oss-vs-enterprise.md).
- <a id="platform-layout"></a>**Platform layout:** [docs/platform-layout.md](docs/platform-layout.md).
- <a id="repository"></a>**Repository:** [docs/repository-layout.md](docs/repository-layout.md).
- <a id="prerequisites"></a><a id="build"></a>**Prerequisites and build:** [docs/prerequisites-and-build.md](docs/prerequisites-and-build.md); `docs/` and this README are authoritative.

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

<div align="center">

More at **[zyvor.dev/guestkit](https://zyvor.dev/guestkit?utm_source=github&utm_medium=guestkit)** · [docs](https://zyvor.dev/docs?utm_source=github&utm_medium=guestkit) · [blog](https://zyvor.dev/blog?utm_source=github&utm_medium=guestkit)

</div>
