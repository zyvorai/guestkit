# GuestKit Python Bindings

Python bindings for GuestKit — pure-Rust offline disk inspection, assurance scoring, and migration repair.

**PyPI package:** [`zyvor-guestkit`](https://pypi.org/project/zyvor-guestkit/)  
**Wheel filename:** `zyvor_guestkit-*.whl` (underscore, not hyphen)

## Table of Contents

- [Installation](#installation)
- [Quick Start](#quick-start)
- [Assurance APIs (v1.1.0+)](#assurance-apis-v110)
- [Inject payload](#inject-payload)
- [Live guest fix](#live-guest-fix)
- [Guestfs Handle API](#guestfs-handle-api)
- [h2kvm Integration](#h2kvm-integration)
- [Build from Source](#build-from-source)
- [Error Handling](#error-handling)

## Installation

### Prerequisites

- Python 3.10 or later (3.12 recommended on Ubuntu 24.04)
- System tools: `qemu-img`, `qemu-nbd`, `losetup`
- Root or sudo for mount/NBD operations

### PyPI

```bash
pip install zyvor-guestkit
```

### Verify

```python
import guestkit
print(guestkit.__version__)
print(hasattr(guestkit, "run_migrate_repair"))  # True on 1.1.0+
```

## Quick Start

### Assurance (recommended entry point)

```python
import guestkit

# Bootability score before power-on
report = guestkit.run_doctor("vm.qcow2", target="kvm", explain=True)
print(report["bootability"]["score"], report["bootability"]["blockers"])

# Dry-run repair plan
plan = guestkit.run_migrate_repair("vm.qcow2", target="kvm", apply=False)
print(plan["fix_plan"], plan["assessment_score"])

# Apply offline fixes (fstab, GRUB, initramfs, …)
result = guestkit.run_migrate_repair("vm.qcow2", target="kvm", apply=True)
print(result["message"], result["applied"])
```

Inject (hostname, network, users, first-boot) is an extra argument on the same call. See [Inject payload](#inject-payload).

## Assurance APIs (v1.1.0+)

These map 1:1 to CLI commands and are the primary integration surface for **h2kvm** and CI pipelines.

| Python function | CLI equivalent | Returns (dict keys) |
|-----------------|----------------|---------------------|
| `run_doctor(image, target="kvm")` | `guestkit doctor` | `bootability`, `target`, optional `root_cause`, `copilot` |
| `run_boot_inspect(image, target="kvm")` | boot-inspect | `os_release`, `fstab_valid`, `bootloader`, `message` |
| `run_migrate_plan(image, target="kvm")` | `guestkit migrate-plan` | `migration_score`, `bootability`, `fix_plan` |
| `run_repair_plan(image, dry_run=True)` | `guestkit repair --fix boot` | `before_score`, `after_score`, `fix_plan`, `applied` |
| `run_migrate_repair(image, apply=False, inject_json=None)` | `guestkit migrate-repair` | `dry_run`, `applied`, `assessment_score`, `fix_plan`, `notes` |
| `live_fix_commands(...)` | — (run on the booted guest) | `list[str]` of shell commands |
| `run_live_plan(commands, dry_run=False)` | live plan executor | apply result dict |

`inject_json`, `live_fix_commands`, and `run_live_plan` are Python-only. `guestkit migrate-repair` does not take an inject payload.

### Parameters

**`target`** — hypervisor destination: `kvm`, `proxmox`, `qemu`, `hyperv`, `aws`, `azure`, `gcp`, `cloud`, `kubevirt`.

**`run_migrate_repair` options:**

- `apply=False` — dry-run (default); `apply=True` writes changes to disk
- `include_destructive=False` — skip destructive fix steps unless explicitly enabled
- `virtio_win="/path/to/virtio-win.iso"` — Windows VirtIO driver ISO path
- `verbose=True` — include detailed notes in response
- `inject_json` — JSON object (as a string) of extra offline work. Omitted, `""`, or `"null"` is a no-op. Invalid JSON raises `ValueError`.

### Inject payload

h2kvm used to mount the image again after repair to write hostname, network files, users, and first-boot scripts. Pass that work as `inject_json`. GuestKit appends it to the repair plan and writes it on the offline disk. An empty payload adds no operations.

| Field | What it stages |
|-------|----------------|
| `hostname` | `/etc/hostname` |
| `network_files` | `[{ "path", "content" }]`. `path` must be absolute. |
| `users` | Linux: script under `/usr/local/sbin/h2kvm-user-<name>`, run in the guest during apply. Windows: PowerShell staged at `/Windows/Temp/h2kvm-user-<name>.ps1` only (not RunOnce) |
| `services` | Enable the named unit offline (a wants symlink). Does not start it |
| `firstboot` | Script under `/usr/local/sbin/h2kvm-firstboot-N`, then the same chroot-or-first-boot path as Linux users |
| `cloud_init_user_data` | `/var/lib/cloud/seed/nocloud/user-data` |
| `ad_rejoin` | Windows `Add-Computer` script plus a RunOnce key. Domain join password is **not** stored; first boot calls `Get-Credential`. |
| `license_kms` | `slmgr /skms` and `/ato` on first boot |
| `enable_rdp` | Clears `fDenyTSConnections` |

Linux user and first-boot commands are applied in the guest chroot when that works. If the chroot command fails, they are appended to `guestkit-firstboot-live.service` and run on the next boot.

Linux user fields: `name`, `password_hash` (`chpasswd -e`, preferred), `password` (plaintext `chpasswd`), `groups`, `ssh_keys`. Windows users use `password`. Names with `/`, spaces, or newlines are skipped. A Windows user script is only written to disk; schedule it yourself, or use `firstboot` / `ad_rejoin` when you need RunOnce.

```python
import json
import guestkit

payload = {
    "hostname": "app-01",
    "network_files": [
        {
            "path": "/etc/netplan/01-netcfg.yaml",
            "content": "network:\n  version: 2\n  ethernets:\n    eth0:\n      dhcp4: true\n",
        }
    ],
    "users": [
        {
            "name": "migrate",
            "password_hash": "$6$rounds=656000$...",
            "groups": ["sudo"],
            "ssh_keys": ["ssh-ed25519 AAAA... migrate@lab"],
            "os": "linux",
        }
    ],
    "services": ["ssh"],
    "firstboot": ["#!/bin/sh\nsystemctl restart systemd-networkd || true\n"],
}

result = guestkit.run_migrate_repair(
    "vm.qcow2",
    target="kvm",
    apply=False,  # preview the inject ops in fix_plan first
    inject_json=json.dumps(payload),
)
```

Windows rejoin and license reactivation use the same argument:

```python
payload = {
    "ad_rejoin": {
        "domain": "corp.example",
        "ou": "OU=Servers,DC=corp,DC=example",
        "username": "join-account",
    },
    "license_kms": "kms.corp.example",
    "enable_rdp": True,
    "users": [{"name": "breakglass", "password": "change-me", "os": "windows"}],
}
```

### Live guest fix

Offline repair cannot regenerate the initramfs or rewrite GRUB the way a running guest can. `live_fix_commands` returns the shell lines to run **on the guest** (over SSH, or locally if Python is already inside the guest):

```python
cmds = guestkit.live_fix_commands(
    update_grub=True,          # default
    regen_initramfs=True,      # default
    remove_vmware_tools=False, # default; set True to purge open-vm-tools
)
# cmds is a list of shell strings. Run them yourself, or:
guestkit.run_live_plan(cmds, dry_run=True)
guestkit.run_live_plan(cmds, dry_run=False)
```

`run_live_plan` executes on **this machine** through the live plan executor. It does not SSH. Each command expects exit code 0 and times out after 300 seconds. `dry_run=False` is the default.

### Example: CI gate in Python

```python
import sys
import guestkit

report = guestkit.run_doctor("artifact.qcow2", target="kvm")
score = report["bootability"]["score"]
if score < 80:
    print(f"FAIL: bootability {score} < 80", file=sys.stderr)
    sys.exit(1)
print(f"PASS: bootability {score}")
```

## Guestfs Handle API

Low-level GuestFS-compatible handle for custom inspection scripts:

```python
from guestkit import Guestfs

g = Guestfs()
g.add_drive_ro("/path/to/disk.qcow2")
g.launch()

roots = g.inspect_os()
if roots:
    root = roots[0]
    print(g.inspect_get_distro(root), g.inspect_get_hostname(root))
    for mp, dev in g.inspect_get_mountpoints(root).items():
        g.mount_ro(dev, mp)
    if g.is_file("/etc/fstab"):
        print(g.cat("/etc/fstab"))

g.umount_all()
g.shutdown()
```

See `guestkit.pyi` in the repo root for the full typed surface (100+ methods on `Guestfs`).

## h2kvm Integration

h2kvm wraps these calls in `h2kvm.core.guestkit_client`:

```python
from h2kvm.core import guestkit_client
guestkit_client.migrate_repair("/var/lib/h2kvm/out.qcow2", target="kvm", apply=True)
```

Full integration guide: [hyper2kvm-integration.md](../features/hyper2kvm-integration.md).

## Build from Source

```bash
git clone https://github.com/zyvorai/guestkit
cd guestkit
pip install maturin

# Editable install (development)
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin develop --features python-bindings

# Release wheel
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin build \
  --release --features python-bindings --out dist
pip install dist/zyvor_guestkit-*.whl
```

On Python 3.13+, set `PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1` until PyO3 stable ABI catches up.

## Error Handling

Assurance functions raise Python exceptions on hard failures (missing image, mount failure, invalid target). Inspect return dicts for soft failures:

```python
result = guestkit.run_migrate_repair("disk.qcow2", apply=True)
if not result.get("applied") and result.get("dry_run"):
    print("Dry-run only — no changes written")
for note in result.get("notes", []):
    print(note)
```

## See Also

- [hyper2kvm-integration.md](../features/hyper2kvm-integration.md)
- [migration-assurance.md](../features/migration-assurance.md)
- [getting-started.md](getting-started.md)
- [h2kvm GuestKit docs](https://github.com/zyvorai/h2kvm/blob/main/docs/architecture/GUESTKIT.md)
