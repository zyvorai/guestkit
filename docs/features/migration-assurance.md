# Migration assurance platform

GuestKit treats each disk image as a **digital twin**: an offline `EvidenceSnapshot` plus scoring engines that answer “will this VM boot?” and “what must change before cutover?” — without powering the guest on.

## Architecture

```text
Disk image (QCOW2/VMDK/…)
        │
        ▼
  guestfs mount (read-only)
        │
        ▼
  EvidenceSnapshot          ← fstab, modules, VM tools, Windows signals, …
        │
        ├─► BootabilityReport   (doctor / boot engine)
        ├─► MigrationScoreReport (migrate-plan)
        ├─► Policy validation   (policy check + expression DSL)
        ├─► Fleet clusters      (fleet analyze)
        ├─► Migration waves     (fleet wave-plan)
        ├─► Drift vs baseline   (fleet watch)
        ├─► FixPlan             (repair --fix boot)
        └─► GuestKitQemuPlan    (guestkit-qemu plan / run)
```

| Module | Role |
|--------|------|
| `src/evidence/` | Normalized snapshot schema (`EvidenceSnapshot`, v1) |
| `src/boot/` | Weighted bootability checks, blockers, warnings |
| `src/cli/migrate/plan.rs` | Hypervisor-aware migration scoring |
| `src/inference/` | Root-cause chain for `--explain` |
| `src/fleet/analyzer.rs` | Cluster identical VMs, snowflakes, blockers |
| `src/fleet/wave.rs` | Dependency-aware migration wave ordering |
| `src/fleet/baseline.rs` | Per-VM golden baseline storage for `fleet watch` |
| `src/ai/drift.rs` | Semantic drift explanation (baseline vs current) |
| `src/cli/plan/` | Fix plans — security profiles **and** boot repair |
| `src/qemu/` | Declarative QEMU/VirtIO config + assurance-gated launcher |

Evidence is cached under `~/.cache/guestkit/` when `doctor` runs successfully.

> **Not legacy appliance tooling:** Assurance uses GuestKit's pure Rust disk stack. No `legacy guest tools` or `guestkit` required.

### `run_boot_inspect` / Zyvor HTTP API

Rust API and Zyvor route for stopped KubeVirt VMs (Zeus OS Guest Intelligence):

```rust
use guestkit::run_boot_inspect;
let summary = run_boot_inspect(path, "kubevirt", false)?;
```

```bash
curl "$ZYVOR/api/v1/kubevirt/vms/default/my-vm/boot-inspect"
```

See [kubevirt-integration.md](kubevirt-integration.md).

### Assured QEMU launch

`guestkit-qemu` consumes the same `EvidenceSnapshot` + `BootabilityReport` and
refuses to start when blockers remain, the score is below `--min-boot-score`, or
a UEFI guest has no pflash firmware (unless `--allow-unready`).

```bash
guestkit-qemu plan vm.qcow2 --json
guestkit-qemu run vm.qcow2 --min-boot-score 80 --qmp-socket /run/guestkit/vm.qmp
```

See [qemu-runtime.md](qemu-runtime.md).

### TUI parity

The **Assurance** view in `guestctl tui` reuses the same evidence → boot → migration pipeline as `doctor` and `migrate-plan` (no second guest mount when the TUI already has guestfs open). Open it from the Security group, the command palette (`doctor`, `goto assurance`), or the dashboard boot line. Keys: `d` refresh doctor, `t` cycle target (`kvm` / `proxmox` / `aws`), `e` export fix plan YAML to the current directory. Configure `default_migration_target` and `assurance_on_startup` under `[behavior]` in `tui.toml`.

## Commands

### `guestkit doctor` — boot assurance score

Predicts first-boot success on a target hypervisor before migration.

```bash
guestkit doctor vm.qcow2 --target kvm
guestkit doctor vm.vmdk --target proxmox --explain
guestkit doctor vm.qcow2 --target kvm -o json
guestkit doctor vm.qcow2 --target kvm -o json --fail-below 80
```

| Flag | Description |
|------|-------------|
| `--target` | `kvm`, `proxmox`, `qemu`, `hyperv`, `aws`, `azure`, `gcp`, `cloud` |
| `--explain` | Root-cause chain from inference engine |
| `-o json` | Machine-readable `bootability` + optional `root_cause` |
| `--fail-below` | Exit code `1` if boot score is below threshold (0–100); JSON still printed |

Output includes a **boot assurance score** message, **blockers** (with remediation hints), **warnings**, and per-check pass/fail lines. Windows guests also run **BOOT-012/013/014** (EFI bootmgr, BCD, System Reserved / ESP layout) and migration checks **MIG-W-005/009** (BitLocker, VSS), **MIG-W-006/007/008** (ghost NICs, static IPs, activation/OEM), plus **MIG-W-012/013** (hotfixes/servicing, VirtIO `.sys` files).

#### CI gate example

```yaml
# .github/workflows/vm-assurance.yml
- name: Boot assurance gate
  run: |
    guestkit doctor vm-images/migrated.qcow2 \
      --target proxmox \
      -o json \
      --fail-below 80 \
      > boot-report.json
```

### `guestkit migrate-repair` — assessment → FixPlan

Turns failed migration checks into an auditable FixPlan (preview by default; `--apply` runs offline apply).

```bash
guestkit migrate-repair win.qcow2 --target kvm
guestkit migrate-repair win.qcow2 --target kvm --export repair.yaml
guestkit migrate-repair win.qcow2 --target kvm --apply --yes

# Offline VirtIO driver inject: point at an extracted virtio-win tree
export GUESTKIT_VIRTIO_WIN=/path/to/virtio-win
guestkit migrate-repair win.qcow2 --target kvm --apply --yes

# Or pass the tree / driver dir explicitly
guestkit migrate-repair win.qcow2 --target kvm \
  --virtio-win /path/to/virtio-win --apply --yes
```

| Flag | Description |
|------|-------------|
| `--target` | Target hypervisor (required) |
| `--export FILE` | Write FixPlan JSON/YAML |
| `--apply` | Apply offline (requires confirmation / `--yes`) |
| `--destructive` | Include non-undoable ops (ghost NIC, tools uninstall) |
| `--virtio-win DIR` | Host path for `DriverInject` (`GUESTKIT_VIRTIO_WIN` also works) |

`DriverInject` resolve order: plan `host_dir` → `--virtio-win` / planner override → `$GUESTKIT_VIRTIO_WIN/<driver>` (common amd64 / 2k22 / w10 layouts). Build with `--features registry-write,agent` for offline inject.

### `guestkit passport` — Cutover Passport (CI gate)

Packages evidence digest, boot + migration scores, critical blockers, FixPlan digest, Windows offline flags (BitLocker hard-block), optional live agent attestation, and suite handoff (HyperSDK → hyper2kvm). This is the artifact ops/security accept before convert — not a virt-v2v replacement.

```bash
guestkit passport emit vm.qcow2 --target kvm -o passport.json --bundle
guestkit passport verify passport.json --fail-below 80

# Optional: live attestation via agent-proxy
guestkit passport emit vm.qcow2 --target kvm -o p.json \
  --live-url http://127.0.0.1:8765

# Optional: Ed25519 sign (requires --features agent)
guestkit passport keygen --seed ./ed25519.seed --public ./ed25519.pub
guestkit passport emit vm.qcow2 --target kvm -o p.json \
  --sign-key ./ed25519.seed --issuer "ci/prod" --expires-hours 72
guestkit passport verify p.json --fail-below 80 --require-signature \
  --trust-keys ./trusted-pubs.txt --max-age-hours 168
```

| Flag | Description |
|------|-------------|
| `emit --target` | Target hypervisor (required) |
| `emit -o FILE` | Passport JSON path |
| `emit --bundle` | Also write `<stem>.passport/` with companion FixPlan YAML |
| `emit --content-hash` | SHA-256 of image bytes (slow) |
| `emit --virtio-win DIR` | VirtIO tree for DriverInject planning |
| `emit --live-url URL` | Agent-proxy base for `/doctor` live attestation |
| `emit --sign-key FILE` | Ed25519 seed (32 bytes or 64 hex); needs `agent` feature |
| `emit --issuer NAME` | Record issuer identity on the passport |
| `emit --expires-hours N` | Set `expires_at` to N hours from emit time |
| `keygen --seed/--public` | Generate Ed25519 seed + public hex (needs `agent` feature) |
| `verify --fail-below N` | Fail if min(boot, migration) score is below N |
| `verify --require-signature` | Require valid Ed25519 signature |
| `verify --trust-keys FILE` | Allowlist of Ed25519 public key hex (one per line) |
| `verify --max-age-hours N` | Reject passports older than N hours |

Web console: dock **Passport** enqueues `POST /api/v1/vms/:id/passport` and downloads the JSON.

### `guestkit migrate-plan` — hypervisor-aware migration score

Builds on the same evidence + boot report, then applies target-specific rules (VirtIO drivers, cloud-init, VMware Tools removal, BitLocker, SELinux relabel, etc.).

```bash
guestkit migrate-plan vm.vmdk --target proxmox
guestkit migrate-plan vm.qcow2 --target aws --explain -o json
guestkit migrate-plan vm.vmdk --target proxmox --export migration-plan.yaml
```

| Flag | Description |
|------|-------------|
| `--target` | Target hypervisor (required) |
| `--explain` | Root-cause chain from inference engine |
| `--export FILE` | Write executable fix plan (`.yaml` or `.json`) |
| `-o json` | Machine-readable score + checklist |

**Target mapping (examples)**

| `--target` | Boot analysis | Migration rules |
|------------|---------------|-----------------|
| `kvm`, `proxmox`, `qemu` | Proxmox/KVM | VirtIO, virtio-scsi/net, VMware Tools → qemu-ga |
| `aws`, `azure`, `gcp`, `cloud` | Cloud | cloud-init datasource, licensing (BYOL) |
| `hyperv`, `hyper-v` | Hyper-V | Hyper-V-specific checks |

### `guestkit policy check` — policy-as-code

Alias over validation with an **expression DSL** over evidence fields, e.g. `bootability.score >= 80`. Use `--policy policy.yaml` or built-in benchmarks.

```bash
guestkit policy check vm.qcow2 --policy cis.yaml
guestkit policy check vm.qcow2 --benchmark cis -o json
```

### `guestkit fleet analyze` — fleet posture

Scans a directory of disk images, clusters identical OS fingerprints, flags snowflakes and low boot-score blockers.

```bash
guestkit fleet analyze ./vms/
guestkit fleet analyze ./vms/ --recursive -j 4
guestkit fleet analyze ./vms/ -o json
```

Parallelism defaults to `min(4, CPUs)` (override with `-j` / `--jobs` or `GUESTKIT_FLEET_JOBS`). Evidence-cache hits skip remounting.

### `guestkit fleet wave-plan` — dependency-aware migration ordering

Orders a fleet's disk images into migration **waves**: a batch of VMs that
can safely move together, followed by the next batch, and so on. Two
signals from the same evidence used by `fleet analyze` decide ordering:

- **Role** — a VM running a database-ish systemd unit (`postgresql`,
  `mysql`, `mariadb`, `mongod`, `redis`, `cassandra`) is treated as
  infrastructure other VMs may depend on, and is pulled earlier within its
  wave.
- **Storage dependency** — an `/etc/fstab` NFS mount whose server hostname
  matches another VM in the same fleet becomes a `depends_on` edge; the
  client VM is placed in a strictly later wave than its NFS server.

Wave assignment is a level-batched topological sort (Kahn's algorithm) over
those edges — VMs with no dependencies land in wave 0, VMs that depend only
on wave-0 VMs land in wave 1, and so on. A dependency **cycle** (VM A's NFS
server is VM B, and B's server is A) can't be topologically ordered, so
those VMs are reported separately under `cycles` instead of being silently
dropped or arbitrarily ordered.

```bash
guestkit fleet wave-plan ./vms/
guestkit fleet wave-plan ./vms/ --recursive -j 4
guestkit fleet wave-plan ./vms/ -o json
```

This only orders *within* the fleet directory scanned — it does not infer
dependencies on infrastructure outside the given VMs (e.g. an external
database VM not included in the scan), and it does not attempt
application-level dependency discovery (no port/socket probing, no config
file parsing beyond fstab).

### `guestkit fleet watch` — scheduled drift monitoring

Diffs each VM's current evidence against a stored **golden baseline**,
reusing the same semantic drift explanations `forensic-diff` uses for
single-VM before/after comparisons (`src/ai/drift.rs`). Unlike
`fleet analyze`/`wave-plan`, this command has memory across invocations —
the first run against a given image establishes its baseline (one JSON
file per VM under `dirs::cache_dir()/guestkit/fleet-baseline`, override
with `GUESTKIT_FLEET_BASELINE_DIR`); every run after that diffs current
evidence against the *same* stored baseline instead of silently rolling
forward to whatever was last seen — a monitor that quietly re-baselines on
every run would never report the drift it exists to catch.

```bash
guestkit fleet watch ./vms/                    # first run: establishes baselines
guestkit fleet watch ./vms/                    # later runs: reports drift, if any
guestkit fleet watch ./vms/ --reset-baseline    # accept current state as the new baseline
guestkit fleet watch ./vms/ --fail-on-drift     # non-zero exit if any VM drifted (cron/CI gating)
guestkit fleet watch ./vms/ -o json
```

Intended to be invoked on a schedule by an external scheduler (cron,
systemd timer, or a Kubernetes `CronJob` — see
`deploy/helm/zyvor/templates/fleet-drift-watch-cronjob.yaml`) rather than
run its own internal loop, the same way `doctor`/`fleet analyze` are
scheduler-agnostic, single-shot commands. `--fail-on-drift` makes it
composable with the same "gate a pipeline on a non-zero exit" pattern
`passport verify --fail-below` uses.

### `guestkit forensic-diff` — security drift

Compares two snapshots (before/after incident, golden vs drifted) for config drift, suspicious persistence, and ransomware indicators.

```bash
guestkit forensic-diff before.qcow2 after.qcow2 -o json
```

### `guestkit repair --fix boot` — transactional boot repair

Converts doctor blockers/warnings into a **fix plan**, applies it with backup semantics, then re-runs doctor to show score delta.

```bash
guestkit repair vm.qcow2 --fix boot --dry-run   # preview operations
guestkit repair vm.qcow2 --fix boot             # apply + re-score
```

Plans are tagged `boot` / `doctor` and generated via `PlanGenerator::from_boot_report`.

### `guestkit inspect --profile windows-migration`

Deep Windows signals for migration: BitLocker, domain join, RDP, hypervisor remnants, driver gaps (SAM/SECURITY hive parsing).

```bash
guestkit inspect win.vmdk --profile windows-migration -o json
```

## Recommended workflow

```bash
# 1. Boot gate
guestkit doctor source.vmdk --target proxmox --explain

# 2. Migration checklist + export fix plan
guestkit migrate-plan source.vmdk --target proxmox -o json > plan.json
guestkit migrate-plan source.vmdk --target proxmox --export migration-fix-plan.yaml

# 3. Windows-specific inventory (if applicable)
guestkit inspect source.vmdk --profile windows-migration -o json

# 4. Policy sign-off
guestkit policy check source.vmdk --policy migration-policy.yaml

# 5. Fleet context (many disks)
guestkit fleet analyze ./exports/

# 6. Fix blockers offline, then re-doctor
guestkit repair source.vmdk --fix boot --dry-run
guestkit repair source.vmdk --fix boot
guestkit doctor source.vmdk --target proxmox

# 7. Hand off to h2kvm / hypervisor import
h2kvmctl local --vmdk source.vmdk --to-output out.qcow2 --backend guestkit
```

## Python API (v1.1.0+)

Same assurance engine as CLI — used by **h2kvm** offline fixer:

```python
import json
import guestkit

report = guestkit.run_doctor("source.vmdk", target="kvm", explain=True)
plan = guestkit.run_migrate_plan("source.vmdk", target="kvm", export_fix_plan=True)
guestkit.run_migrate_repair(
    "source.qcow2",
    target="kvm",
    apply=True,
    inject_json=json.dumps({"hostname": "app-01"}),
)
```

`inject_json` is Python-only (hostname, network files, users, services, first-boot, cloud-init, AD rejoin, KMS, RDP). The CLI `migrate-repair` command does not accept it. After boot, `live_fix_commands()` returns the initramfs/GRUB shell lines to run on the guest.

See [python-bindings.md](../user-guides/python-bindings.md) and [hyper2kvm-integration.md](hyper2kvm-integration.md).

## Relationship to fix plans

| Plan source | Profile | Use case |
|-------------|---------|----------|
| Security profile | `security` | Hardening from inspect findings |
| Doctor boot report | `boot-repair` | Boot blockers from `repair --fix boot` |
| Migration profile | `migration` | Manual/runbook plans (see [fix-plans.md](fix-plans.md)) |

`migrate-plan` is **scoring and guidance** by default; use **`--export`** to produce an executable fix plan, or **`repair --fix boot`** for boot blockers only.

## Library API (Rust)

```rust
use guestkit::evidence::build_evidence;
use guestkit::boot::{analyze_bootability, BootTarget};
use guestkit::cli::migrate::plan::compute_migration_score;

// After guestfs mount: build_evidence → analyze_bootability → compute_migration_score
```

## See also

- [Guest agent](guest-agent.md) — live in-guest assurance via virtio-serial
- [Zyvor GuestKit](https://zyvor.dev/guestkit) — platform overview
- [VM migration guide](../user-guides/vm-migration.md) — fstab, registry, h2kvm handoff
- [Fix plans](fix-plans.md) — preview, export, apply
- [Security profiles](../user-guides/profiles.md) — migration and windows-migration profiles
- [Changelog](../development/CHANGELOG.md) — v0.3.5+ assurance CLI; v0.3.6 TUI parity
