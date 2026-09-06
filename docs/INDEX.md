# GuestKit documentation

Operator cheat sheets (cutover cookbook, day-0/rescue, Passport, env vars): **[GitHub Wiki](https://github.com/zyvorai/guestkit/wiki)**.

## Start here

| Guide | Description |
|-------|-------------|
| [Getting started](user-guides/getting-started.md) | Build, install, first commands |
| [CLI reference](user-guides/cli-guide.md) | Command index → topic guides & `guestkit --help` |
| [Quick reference](user-guides/quick-reference.md) | Cheat sheet |
| [Dump virsh](user-guides/virsh-to-guestkit.md) | Replace `virsh qemu-agent-command` with `guestkit qga` |
| [FAQ](user-guides/faq.md) | Common questions |
| [Troubleshooting](user-guides/troubleshooting.md) | Fixes for common issues |

## Features

| Guide | Description |
|-------|-------------|
| [TUI dashboard](features/tui-enhancements.md) | Two-tier tabs, **Assurance** (doctor/migrate-plan), fix-plan preview — [zyvor.dev/guestkit](https://zyvor.dev/guestkit) |
| [Web Image Vault](user/using-the-dashboard.md) | OSS zyvor-ui: inventory tabs, Assurance, Profiles, Files browse |
| [File explorer](features/explore/EXPLORE-QUICKSTART.md) | `guestkit explore` (+ web Browse files) |
| [Interactive shell](user-guides/interactive-mode.md) | REPL mode |
| [Security profiles](user-guides/profiles.md) | Security, compliance, migration profiles |
| [Migration assurance](features/migration-assurance.md) | Doctor, migrate-plan, fleet, policy, forensic diff |
| [QEMU / VirtIO runtime](features/qemu-runtime.md) | Assured `guestkit-qemu plan|run|qmp` from evidence |
| [VM lifecycle](features/vm-runtime.md) | Lab `guestkit vm`; production run/network/TTL → [FluxVM](https://github.com/zyvorai/fluxvm) |
| [Industry use cases](INDUSTRY_USE_CASES.md) | Real-world scenarios, PM/TA view, Zyvor product stack |
| [Fix plans](features/fix-plans.md) | Offline patch workflow |
| [Export formats](features/export-formats.md) | JSON, YAML, HTML, PDF |
| [Python bindings](user-guides/python-bindings.md) | Assurance APIs + Guestfs handle (v1.1.0+) |
| [h2kvm integration](features/hyper2kvm-integration.md) | Convert/deploy pipeline with h2kvm |
| [Handoff / quarantine](user-guides/handoff-quarantine.md) | Passport → h2kvmctl job, fleet quarantine, virtctl plugin |
| [VM migration](user-guides/vm-migration.md) | End-to-end migration handoff |
| [KubeVirt + Zeus OS](features/kubevirt-integration.md) | In-cluster boot inspect API (pure Rust, not legacy appliance tooling) |
| [Replace virtctl guestfs](features/virtctl-guestkit.md) | `virtctl-guestkit guestfs` on a PVC — GuestKit, not libguestfs |
| [Guest agent](features/guest-agent.md) | In-guest agent + host `guestkit qga` / `agent-call` |
| [Guest Control Fabric](features/guest-control-fabric.md) | Transport ladder, airgap QGA install, Agent Doctor, capability contract |
| [Dump virsh](user-guides/virsh-to-guestkit.md) | Command map: QGA / inspect / doctor replace virsh; lifecycle stays virtctl/Machina |
| [img / firstboot / virtio-win](user-guides/img-firstboot.md) | qemu-img wrapper, domain disk parse, virtio-win plan, first-boot gate |
| [Rescue / SBOM CI](devops/10-rescue-sbom-ci.md) | Rescue dry-run Action, `sbom-diff`, forensic-diff SBOM attach |
| [Cloud profiles / Rego](user-guides/cloud-policy-rego.md) | AWS/Azure/GCP/OpenStack policy packs + `policy rego` |
| [Cloud-init datasource](user-guides/cloud-init.md) | Offline Ec2/Azure/GCE/NoCloud pin + seed |
| [Cutover bundle](user-guides/cutover-bundle.md) | gate + sysprep/SELinux/BitLocker + cloud-profile/rego + virtctl resolve |
| [SELinux / sysprep / BitLocker](user-guides/cutover-prep.md) | Offline relabel, Windows generalize prep, recovery-key escrow |

## Deployment

| Guide | Description |
|-------|-------------|
| [**DevOps runbooks**](devops/README.md) | Passport CI gate, repair worker, air-gap packages, fleet, cutover weekend, triage |
| [Run from GHCR](guides/DOCKER.md#published-images-ghcr) | Pull `ghcr.io/zyvorai/*` images, `docker compose up`, or Helm |
| [Docker](guides/DOCKER.md) | Container usage (web stack + CLI) |
| [Cloud disk sources](guides/cloud-disk-sources.md) | S3 / GCS / Azure URI pulls + cache + CI recipe |
| [Remote deploy](guides/DEPLOY-REMOTE.md) | SSH deploy to Linux hosts + web console access |
| [RPM build](development/RPM-BUILD.md) | Fedora/RHEL packages |

## Architecture & project

| Guide | Description |
|-------|-------------|
| [Architecture](architecture/overview.md) | How GuestKit is structured |
| [Roadmap](development/roadmap.md) | Shipped Unreleased slices; issue-driven next work |
| [Changelog](development/CHANGELOG.md) | Version history |
| [Contributing](development/CONTRIBUTING.md) | How to contribute |

## Zyvor / Enterprise

| Guide | Description |
|-------|-------------|
| [Open source vs Enterprise](ce-vs-enterprise.md) | OSS engine vs GuestKit Enterprise control plane |
| [Enterprise guide](zyvor-enterprise.md) | Sales, SLAs, full HyperSDK suite |

## Examples

See [`examples/`](../examples/) in the repository root.
