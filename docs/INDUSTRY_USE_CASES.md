# GuestKit — industry use cases & platform context

**Product:** Offline VM intelligence and migration assurance

Cross-reference: [User stories](USER_STORIES.md) · [Migration assurance](features/migration-assurance.md) · [README](../README.md)

---

## What GuestKit does (simple terms)

Before you migrate a VM from VMware, Hyper-V, Nutanix, or public cloud to **KVM, Proxmox, KubeVirt, AWS, Azure, GCP**, or private OpenStack, GuestKit inspects the **disk image without booting the VM** and answers:

| Question | GuestKit capability |
|----------|---------------------|
| Will this VM boot after migration? | `guestkit doctor` — boot assurance score, blockers, warnings |
| What drivers/configs are missing? | Evidence snapshot + migration scoring |
| What needs fixing before cutover? | Blockers with remediation hints, `--explain` root-cause |
| Can we generate a fix/migration plan? | `guestkit migrate-plan --export plan.yaml` |
| Can we scan a whole fleet of images? | `guestkit fleet analyze` |

GuestKit is the **pre-flight assurance layer**. It does not replace hyper2kvm, Proxmox import, or KubeVirt — it de-risks migration **before** those tools run.

---

## Real industry use cases

### 1. VMware → Proxmox / KVM migration

A company wants to reduce VMware licensing cost and move hundreds of VMs to Proxmox or KVM.

GuestKit inspects `.vmdk` / `.qcow2` files offline and checks VirtIO drivers, bootloader, fstab, disk layout, and migration blockers.

**Business value:** Fewer failed migrations, less weekend firefighting, predictable cutover planning.

```bash
guestkit doctor app-server.vmdk --target proxmox --explain
guestkit migrate-plan app-server.vmdk --target proxmox --export plan.yaml
```

### 2. Bank or insurance data-center modernization

Banks run legacy Windows/Linux VMs with strict downtime windows. Before moving workloads to a private cloud, teams need assurance that every VM will boot after migration.

GuestKit scores boot readiness and surfaces BitLocker, missing drivers, domain/RDP issues, VMware Tools dependency, boot partition problems, and Linux mount issues.

**Business value:** Classify VMs into green / yellow / red **before** touching production.

### 3. Healthcare or government offline compliance

Regulated sectors cannot freely boot production images in test environments (patient, citizen, or classified data).

GuestKit inspects disk images **offline** — OS version, filesystem, bootability, security posture, migration readiness — without powering on the guest.

### 4. Cloud migration factory

Enterprises and migration partners receive thousands of VM exports. They need automation to gate which VMs are ready for AWS, Azure, GCP, or private cloud.

```bash
guestkit doctor user-vm.qcow2 --target aws -o json --fail-below 80
```

**Business value:** Repeatable **migration gate** in CI/CD instead of manual VM checks.

### 5. KubeVirt / OpenShift Virtualization readiness

Teams moving VMs into Kubernetes-based virtualization validate **stopped** VM disks before import.

GuestKit pairs with the in-repo **Zyvor web stack** and KubeVirt hooks (`crates/zyvor-api`, `docs/features/kubevirt-integration.md`) for API-style boot inspect on stopped VMs.

### 6. Managed service provider (MSP) migration assessment

MSPs run GuestKit on user exports and deliver readiness reports: assurance score, blockers, warnings, target recommendation, fix plan, migration risk.

**Business value:** Paid **migration assessment** before the migration project.

### 7. Security forensics and drift detection

`guestkit fleet analyze` and forensic diff compare images before/after incidents or against golden baselines.

**Use case:** Config drift, suspicious persistence, changed boot files, ransomware-related indicators.

### 8. Manufacturing / retail edge VM migration

Factories and stores run legacy VMs for POS, inventory, and machine control. Downtime is expensive.

GuestKit inspects disk images offline before moving workloads to newer edge infrastructure.

**Business value:** Prevents branch / store / factory outages from failed boot after migration.

---

## Product manager view

| Buyer / user | Why GuestKit |
|--------------|--------------|
| Migration engineer | Pre-cutover confidence |
| Cloud transformation team | Batch readiness at scale |
| SRE / platform team | Fleet drift + CI gates |
| MSP migration team | Assessment deliverable |
| Private cloud / KubeVirt team | Stopped-VM validation |
| Security / forensics team | Offline evidence, forensic diff |

**Core value proposition:**

> Know whether a VM will boot after migration **before** you actually migrate it.

Failed VM migrations are expensive, stressful, and usually happen in tight downtime windows. GuestKit shifts discovery **left** — offline, automatable, and CI-friendly.

---

## Technical architect view

### Enterprise assurance pipeline

```text
VM exports (VMDK / QCOW2 / RAW)
        ↓
GuestKit offline inspection
        ↓
Evidence snapshot
        ↓
Boot assurance score + migration score + blockers
        ↓
Fix plan / policy report / JSON output
        ↓
Migration platform: hyper2kvm · Proxmox · KubeVirt · cloud import
```

GuestKit sits **above** the migration executor — assurance in, validated images out.

### Where GuestKit fits in the Zyvor / HyperSDK suite

From [zyvor.dev](https://zyvor.dev) — **15 products, one API fabric**, one operating model for export → convert → inspect → deploy → operate:

```text
Export          HyperSDK Platform
    ↓
Convert         hyper2kvm
    ↓
Inspect         GuestKit          ← you are here
    ↓
Build           Veyron
    ↓
Deploy          Aether
    ↓
Manage          Zeus OS
    ↓
Automate        Ragnarok
    ↓
Monitor         PacketWolf
    ↓
Compute         Forge
    ↓
Provision       IronWolf
    ↓
Cluster         HyperCluster
```

Supporting host and fabric layers: **Machina** (libvirt/KVM hypervisor OS), **Zyvor Fabric** (systemd-native private cloud), **Hermes** (application layer on Kubernetes).

---

## Zyvor product stack (from [zyvor.dev](https://zyvor.dev))

| Product | Role | Link |
|---------|------|------|
| **HyperSDK Platform** | Multi-cloud VM export (11+ providers), APIs, scheduling | [zyvor.dev/hypersdk](https://zyvor.dev/hypersdk) |
| **hyper2kvm** | Hypervisor → KVM conversion, VirtIO/boot fixes, validation | [zyvor.dev/hyper2kvm](https://zyvor.dev/hyper2kvm) |
| **GuestKit** | Offline guest disk inspect, repair, migration assurance | [zyvor.dev/guestkit](https://zyvor.dev/guestkit) |
| **Veyron** | KubeVirt VM command center, declarative templates | [zyvor.dev/veyron](https://zyvor.dev/veyron) |
| **Aether** | Universal runtime — Podman, K8s, bare metal; confidential computing | [zyvor.dev/aether](https://zyvor.dev/aether) |
| **Zeus OS** | Cloud / KubeVirt control plane, VM fleet day-2 | [zyvor.dev/zeus-os](https://zyvor.dev/zeus-os) |
| **Hermes** | Application layer for Kubernetes | [zyvor.dev/hermes](https://zyvor.dev/hermes) |
| **Machina** | Physical hypervisor OS (libvirt/KVM), OpenStack day-2 | [zyvor.dev/machina](https://zyvor.dev/machina) |
| **Zyvor Fabric** | systemd-native private cloud | [zyvor.dev/zyvor-fabric](https://zyvor.dev/zyvor-fabric) |
| **Ragnarok** | AI infrastructure automation on Kubernetes | [zyvor.dev/ragnarok](https://zyvor.dev/ragnarok) |
| **PacketWolf** | Kernel-native network intelligence (eBPF) | [zyvor.dev/packetwolf](https://zyvor.dev/packetwolf) |
| **Forge** | GPU fabric on Kubernetes | [zyvor.dev/forge](https://zyvor.dev/forge) |
| **IronWolf** | Bare-metal lifecycle (Metal3) | [zyvor.dev/ironwolf](https://zyvor.dev/ironwolf) |
| **HyperCluster** | Bare-metal Kubernetes bootstrap | [zyvor.dev/hypercluster](https://zyvor.dev/hypercluster) |

### Suite categories (zyvor.dev)

| Category | Products |
|----------|----------|
| **Migration & export** | HyperSDK Platform, hyper2kvm, **GuestKit** |
| **Kubernetes & VM management** | Zeus OS, Veyron, Ragnarok, Machina, Zyvor Fabric |
| **Networking & observability** | Aether, PacketWolf |
| **Compute & bare metal** | Forge, IronWolf, HyperCluster |

### Supported migration sources & targets

VMware vSphere · Nutanix AHV · AWS · Azure · Google Cloud · Hyper-V · Oracle Cloud · OpenStack · Alibaba Cloud · Proxmox · KubeVirt · KVM

### Open source vs enterprise

| | Open source (this repo) | Enterprise ([zyvor.dev](https://zyvor.dev)) |
|---|------------------------|-----------------------------------------------|
| GuestKit CLI/TUI | ✅ | Same codebase + SLA |
| Self-hosted web stack | ✅ GHCR / Helm | Hardened reference architectures |
| Migration programs | DIY / community | VMware exit, air-gap, regulated fleets |
| Support | GitHub | [sales@zyvor.dev](mailto:sales@zyvor.dev) |

See [ce-vs-enterprise.md](ce-vs-enterprise.md) and [zyvor-enterprise.md](zyvor-enterprise.md).

---

## Typical commands by scenario

| Scenario | Commands |
|----------|----------|
| Single VM pre-cutover | `doctor` → `migrate-plan --export` |
| CI migration gate | `doctor -o json --fail-below 80` |
| Fleet readiness | `fleet analyze ./exports --recursive -o json` |
| Interactive review | `guestctl tui vm.qcow2` |
| KubeVirt / Zeus (stopped VM) | Zyvor API boot-inspect — [kubevirt-integration.md](features/kubevirt-integration.md) |
| Full VMware exit pipeline | HyperSDK export → hyper2kvm → **GuestKit** → Zeus OS import |

---

## Further reading

- [User stories & acceptance criteria](USER_STORIES.md)
- [Migration assurance](features/migration-assurance.md)
- [VM migration + hyper2kvm](user-guides/vm-migration.md)
- [Architecture overview](architecture/overview.md)
- [Zyvor suite comparison](https://zyvor.dev/docs/products) · [Suite product guides](https://zyvor.dev/docs/intro#suite-product-guides)
