# GuestKit architecture (v1.1.0)

Offline VM intelligence and migration assurance — **Rust control plane** with host block-device access for mount-heavy operations.

## Positioning (accurate)

| Claim | Reality |
|-------|---------|
| **no appliance daemon** | ✅ No legacy appliance tooling daemon workflow |
| **Pure Rust parsing** | ✅ Partition tables, FS signatures, evidence schema, boot engine, assurance APIs |
| **In-process QCOW2 file read** | Partial — format detection + selective reads; full cluster walk defers to **qemu-nbd** |
| **File access inside guests** | Via **loop devices / qemu-nbd** + host mount (`src/guestfs/`), not in-process ext4/NTFS parsers |
| **Web UI** | ✅ Shipped — `deploy/ui/` (inventory + Assurance + Profiles + Files), GHCR `zyvor-ui` |

**Host dependencies (Linux):** `losetup`, `qemu-nbd` (for QCOW2/VMDK), kernel `nbd`/`loop` modules, optional `qemu-img` for format conversion.

## System layers

```text
┌─────────────────────────────────────────────────────────────────┐
│  Surfaces: guestkit · guestctl · guestkit-qemu · Python · UI    │
├─────────────────────────────────────────────────────────────────┤
│  Assurance: evidence → boot score → migrate-plan → fleet/policy │
│             → GuestKitQemuPlan (gated QEMU/VirtIO launch)       │
├─────────────────────────────────────────────────────────────────┤
│  Live QGA: guestkit qga / agent-call (unix socket; no virsh)    │
├─────────────────────────────────────────────────────────────────┤
│  AI (optional): deterministic intel + `--features ai` LLM agent │
├─────────────────────────────────────────────────────────────────┤
│  guestfs façade: Rust orchestration + NBD/loop + mount        │
├─────────────────────────────────────────────────────────────────┤
│  Disk parsers: MBR/GPT, FS magic, registry/hive (Rust)          │
└─────────────────────────────────────────────────────────────────┘

Parallel platform runtime (same repo):
  zyvor-api (Axum) → Redis job queue → guestkit-worker (privileged)
    jobs: inspect · doctor · migrate-plan · passport · repair · profile · explore · convert · agent.*
  PostgreSQL · KubeVirt client · guest-agent mTLS · PacketWolf hooks
  UI: deploy/ui (static) proxies /api → zyvor-api; optional serve-https.py lab TLS
```

## Repository layout

| Path | Role |
|------|------|
| `src/` | Main `guestkit` crate — CLI, guestfs, boot, evidence, assurance, fleet, TUI, qemu |
| `src/agent/` | In-guest agent + host `qga_client` / `guestkit qga` |
| `src/qemu/` | Declarative QEMU/VirtIO config, GuestKit plan bridge, QMP client |
| `src/ai/` | Deterministic intelligence; LLM agent behind `ai` feature |
| `crates/guestkit-job-spec` | Worker job schema |
| `crates/guestkit-worker` | Redis-queue disk inspection daemon |
| `crates/guestkit-agent-protocol` | Guest agent JSON-RPC framing |
| `crates/zyvor-api` | Web API, auth, KubeVirt, QGA socket ladder (`qga_socket`) |
| `crates/zyvor-guest-agent` | In-guest agent binary |
| `deploy/` | Docker Compose, Helm, UI static assets |
| `k8s/` | KubeVirt-oriented manifests |

## Core data flow

```text
Disk image → guestfs mount (ro) → EvidenceSnapshot
                                        │
                    ├─► BootabilityReport (doctor)
                    ├─► MigrationScoreReport (migrate-plan)
                    ├─► Policy evaluation (policy check)
                    ├─► Fleet clusters (fleet analyze)
                    ├─► FixPlan (repair / plan apply)
                    └─► GuestKitQemuPlan → QEMU argv / QMP
```

Evidence is cached under `~/.cache/guestkit/` after successful `doctor` runs.

## Crates and workspace

Root `Cargo.toml` workspace includes the main package and `guestkit-job-spec`. `zyvor-api` and `guestkit-worker` build as sibling crates with path dependencies (see each crate `Cargo.toml`).

**Version:** 1.2.2 (package) · **License:** Apache-2.0 · **Owner:** ZyvorAI Labs Private Limited

## Security model (web stack)

- **Eval default:** `AUTH_ENABLED=false` in `deploy/docker-compose.ghcr.yml` — localhost demos only
- **Production:** auth on, `JWT_SECRET`, `AGENT_BOOTSTRAP_TOKEN`, Redis password — see [DOCKER.md](../guides/DOCKER.md#production-checklist)
- **OIDC:** ID tokens verified via JWKS (signature, issuer, audience, expiry)

## Further reading

- [Migration assurance](../features/migration-assurance.md)
- [QEMU / VirtIO runtime](../features/qemu-runtime.md)
- [Dump virsh → GuestKit](../user-guides/virsh-to-guestkit.md)
- [Guest agent](../features/guest-agent.md)
- [KubeVirt integration](../features/kubevirt-integration.md)
- [CE vs Enterprise](../ce-vs-enterprise.md)
- [User stories](../USER_STORIES.md)
