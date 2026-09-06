# guestkit Documentation

Offline VM intelligence and migration assurance

## Start Here

| Goal | Document |
|------|----------|
| Getting started | [getting-started.md](user-guides/getting-started.md) |
| Run from GHCR (Docker/Helm) | [guides/DOCKER.md](guides/DOCKER.md#published-images-ghcr) |
| Web Image Vault (OSS UI) | [customer/using-the-dashboard.md](customer/using-the-dashboard.md) |
| CLI guide | [cli-guide.md](user-guides/cli-guide.md) |
| Migration assurance | [migration-assurance.md](features/migration-assurance.md) |
| **QEMU / VirtIO runtime** | [qemu-runtime.md](features/qemu-runtime.md) |
| **Dump virsh → GuestKit** | [virsh-to-guestkit.md](user-guides/virsh-to-guestkit.md) |
| **h2kvm integration** | [hyper2kvm-integration.md](features/hyper2kvm-integration.md) |
| Python bindings | [python-bindings.md](user-guides/python-bindings.md) |
| Roadmap | [roadmap.md](development/roadmap.md) | Shipped Unreleased; issue-driven next |
| Full index | [INDEX.md](INDEX.md) |
| **User journeys & acceptance criteria** | [User Stories](USER_STORIES.md) |
| **Industry use cases & Zyvor stack** | [INDUSTRY_USE_CASES.md](INDUSTRY_USE_CASES.md) |

## User Stories

Persona-based journeys with acceptance criteria: **[USER_STORIES.md](USER_STORIES.md)**

| Persona | Focus |
|---------|-------|
| Alex (Migration Engineer) | Pre-flight VM inspection before cutover |
| Morgan (SRE) | Fleet drift analysis and forensic diff |
| Jordan (Platform Architect) | Boot probability scoring and fix plans |

## Ecosystem

Part of the [Zyvor / HyperSDK platform stack](https://zyvor.dev) — **15 products, one pipeline**. Full industry context: **[INDUSTRY_USE_CASES.md](INDUSTRY_USE_CASES.md)**.

| Product | Role |
|---------|------|
| **HyperSDK Platform** | Multi-cloud VM export & APIs |
| **h2kvm** | Hypervisor → KVM conversion + deploy |
| **guestkit** | Offline VM migration assurance |
| **Veyron** | KubeVirt VM command center |
| **Aether** | Universal runtime portability |
| **Zeus OS** | Cloud / KubeVirt control plane |
| **Hermes** | Application layer for Kubernetes |
| **Machina** | libvirt/KVM hypervisor OS |
| **Zyvor Fabric** | systemd-native private cloud |
| **Ragnarok** | AI ops automation on K8s |
| **PacketWolf** | Kernel-native network intelligence |
| **Forge** | GPU fabric on Kubernetes |
| **IronWolf** | Bare-metal lifecycle (Metal3) |
| **HyperCluster** | Bare-metal Kubernetes bootstrap |

Pipeline: **Export → Convert → Inspect (GuestKit) → Build → Deploy → Manage → Operate** — [zyvor.dev](https://zyvor.dev)
