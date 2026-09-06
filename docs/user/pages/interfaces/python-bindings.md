# Python Bindings

## Purpose

Programmatic access to GuestKit offline disk intelligence — assurance scoring and repair from Python.

## When to use it

- Automate **doctor / migrate-repair** in CI or migration pipelines
- Integrate with **h2kvm** (`h2kvm.core.guestkit_client`)
- Custom inspection scripts via the `Guestfs` handle

## How to get there

- Doc id: `python-bindings`
- Nav: **Interfaces → Python Bindings**
- Primary interface: `pip install hypersdk-guestkit` (PyPI) or `maturin develop --features python-bindings`

## Operate from Python (v1.1.0+)

1. `pip install "hypersdk-guestkit>=1.1.0"` ([PyPI](https://pypi.org/project/hypersdk-guestkit/1.1.0/)). For local builds: `maturin build --release --features python-bindings`.
2. `import guestkit`.
3. **Assurance:** `guestkit.run_doctor("disk.qcow2", target="kvm", explain=True)`.
4. **Repair (dry-run):** `guestkit.run_migrate_repair("disk.qcow2", apply=False)`.
5. **Repair (apply):** `guestkit.run_migrate_repair("disk.qcow2", apply=True)`.
6. **Low-level inspect:** `from guestkit import Guestfs` → `add_drive_ro` → `launch` → `inspect_os`.
7. **Empty / fail:** Import error → wrong package or missing wheel; launch fail → NBD/sudo.
8. **Success:** Bootability score + fix plan JSON; or distro/hostname from Guestfs handle.

Host needs Linux + `qemu-img` / `losetup` / `qemu-nbd`; mount/repair often need root.

See also: [examples/python/assurance_doctor.py](../../../../examples/python/assurance_doctor.py)

## Related pages

- [h2kvm integration](../../../features/hyper2kvm-integration.md)
- [Inspect](../inspection/inspect.md)
- [CLI Guide](../onboarding/cli-guide.md)
- [Guest Files](../guest-files/files.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
