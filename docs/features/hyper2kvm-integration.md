# h2kvm integration

GuestKit provides **offline disk intelligence**; [h2kvm](https://github.com/zyvorai/h2kvm) provides **hypervisor-to-KVM conversion, deployment, and day-2 orchestration**. Together they form the inspect → plan → fix → convert → deploy pipeline.

## Recommended stack (v1.1.0+)

| Layer | Component | Role |
|-------|-----------|------|
| Assurance | GuestKit CLI or Python | Doctor, migrate-plan, migrate-repair, optional `inject_json` |
| Conversion | h2kvm (`h2kvmctl`) | VMDK/OVA → qcow2, flatten, libvirt/KubeVirt deploy |
| Python binding | `zyvor-guestkit` | Native `run_*` functions — **no subprocess wrapper required** |
| Day-2 | Zeus OS / Axiom | Post-cutover operations |

## Python integration (preferred)

Since **v1.1.0**, h2kvm delegates offline repair to GuestKit via PyO3 bindings:

```python
import json
import guestkit

# Dry-run — see what would change
plan = guestkit.run_migrate_repair(
    "/var/lib/h2kvm/input/ubuntu-test.vmdk",
    target="kvm",
    apply=False,
    verbose=True,
)
print(plan["fix_plan"], plan["assessment_score"])

# Apply fstab / GRUB / initramfs fixes offline, plus the payload h2kvm
# used to write in a second mount (hostname, netplan, users, first-boot).
result = guestkit.run_migrate_repair(
    "/var/lib/h2kvm/demo/ubuntu-test/ubuntu-test.qcow2",
    target="kvm",
    apply=True,
    inject_json=json.dumps({
        "hostname": "ubuntu-test",
        "network_files": [
            {"path": "/etc/netplan/01-netcfg.yaml", "content": "network:\n  version: 2\n"}
        ],
    }),
)
print(result["message"], result["applied"])
```

`inject_json` is optional. Omit it, pass `""`, or pass `"null"` and repair behaves as before. GuestKit writes the extra files on the offline disk; h2kvm does not mount the image again. There is no CLI flag for this — `guestkit migrate-repair` stays assessment and boot repair only.

After the guest boots, initramfs and GRUB still have to be refreshed on the running system:

```python
cmds = guestkit.live_fix_commands(remove_vmware_tools=True)
# Run cmds over SSH, or on the guest itself:
guestkit.run_live_plan(cmds, dry_run=False)
```

`run_live_plan` executes on the machine where Python is running. It does not open an SSH session.

h2kvm wraps the same calls in `h2kvm.core.guestkit_client`:

```python
from h2kvm.core import guestkit_client
guestkit_client.migrate_repair(path, target="kvm", apply=True)
```

### Install on migration hosts

```bash
pip install zyvor-guestkit
# h2kvm 1.1.0 — GitHub Release wheel (PyPI pending)
pip install https://github.com/zyvorai/h2kvm/releases/download/v1.1.0/h2kvm-1.1.0-py3-none-any.whl
# or: cd h2kvm && pip install '.[full]'
```

Development wheels from a GuestKit checkout:

```bash
cd guestkit
pip install maturin
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin build \
  --release --features python-bindings --out dist
pip install dist/zyvor_guestkit-*.whl
```

## CLI integration (CI gates, passports)

For CI pipelines and Passport artifacts, use the **GuestKit CLI** directly:

```bash
guestkit doctor disk.qcow2 --target kvm --explain
guestkit migrate-plan disk.qcow2 --target kvm
guestkit migrate-repair disk.qcow2 --target kvm --apply
guestkit passport emit disk.qcow2 --target kvm -o passport.json
guestkit passport handoff passport.json -o job.handoff.yaml --fail-below 80
```

`handoff` refuses (exit 1) on `hard_blocked`, BitLocker, stale/unsigned
passports, or score below `--fail-below`. The YAML is the only input
h2kvm should accept:

```bash
h2kvmctl local --to-output out.qcow2 --backend guestkit \
  --passport passport.json
```

Fleet gate before a wave:

```bash
guestkit fleet quarantine /var/lib/libvirt/images --threshold 80 --fail
```

KubeVirt operators (install `virtctl-guestkit` next to `virtctl`):

```bash
virtctl guestkit doctor --image disk.qcow2 --target kubevirt --explain
virtctl guestkit passport --image disk.qcow2 --target kubevirt -o p.json
virtctl guestkit handoff --passport p.json --fail-below 80
```

## Deploy both projects to a lab host

```bash
# GuestKit CLI (Rust binary)
GUESTKIT_ZYVOR_ACCEPT=1 ./scripts/deploy-remote.sh 175.110.122.71 sus --quick --key

# h2kvm (Python + h2kweb + libvirt stack)
cd /path/to/h2kvm
./scripts/deploy-remote.sh 175.110.122.71 sus --keep-sources
```

See [DEPLOY-REMOTE.md](../guides/DEPLOY-REMOTE.md) (GuestKit) and [h2kvm deploy-remote](https://github.com/zyvorai/h2kvm/blob/main/docs/deployment/deploy-remote.md).

## Assurance API reference

| Python | Returns (dict keys) |
|--------|---------------------|
| `run_doctor(image, target="kvm")` | `bootability`, `target`, optional `root_cause`, `copilot` |
| `run_boot_inspect(image, target="kvm")` | `os_release`, `fstab_valid`, `bootloader`, `message` |
| `run_migrate_plan(image, target="kvm")` | `migration_score`, `bootability`, `fix_plan` |
| `run_repair_plan(image, dry_run=True)` | `before_score`, `after_score`, `fix_plan`, `applied` |
| `run_migrate_repair(image, apply=False, inject_json=None)` | `dry_run`, `applied`, `assessment_score`, `fix_plan`, `notes` |
| `live_fix_commands(update_grub=True, regen_initramfs=True, remove_vmware_tools=False)` | `list[str]` |
| `run_live_plan(commands, dry_run=False)` | live apply result (runs on this machine) |

`bootability` includes `score`, `confidence`, `blockers[]`, `warnings[]`, `checks[]`.

## Legacy subprocess wrapper

The `integration/python/guestkit_wrapper.py` subprocess wrapper remains for older integrations. **New code should use `pip install zyvor-guestkit` and import `guestkit` directly.**

## Validated lab workflow (Ubuntu 24.04)

1. Download osboxes.org VMDK (SourceForge 7z archive)
2. `demo-libvirt.sh ubuntu2404.vmdk ubuntu-test` on h2kvm host
3. GuestKit `run_migrate_repair` applies the offline fix, including any `inject_json` payload
4. Output qcow2 → libvirt domain `ubuntu-test` (credentials: osboxes / osboxes.org)

## Assured local QEMU smoke-test

After convert + repair, smoke-test the qcow2 with GuestKit's assurance gate
before handing off to libvirt/KubeVirt:

```bash
guestkit doctor out.qcow2 --target kvm --fail-below 80
guestkit-qemu run out.qcow2 \
  --min-boot-score 80 \
  --qmp-socket /tmp/out.qmp \
  --ssh-forward 2222
```

GuestKit does not create TAP/bridges; use libvirt or your orchestrator for
production networking. Details: [qemu-runtime.md](qemu-runtime.md).

## See also

- [migration-assurance.md](migration-assurance.md)
- [qemu-runtime.md](qemu-runtime.md)
- [python-bindings.md](../user-guides/python-bindings.md)
- [h2kvm GUESTKIT.md](https://github.com/zyvorai/h2kvm/blob/main/docs/architecture/GUESTKIT.md)
