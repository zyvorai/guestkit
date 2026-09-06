# VM Migration Guide

## Purpose

VM Migration Guide — Guides surface.

## When to use it

- Operate **VM Migration Guide** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `vm-migration`
- Nav: **Guides → VM Migration Guide**
- Primary interface: CLI assurance-first workflow (+ h2kvm convert)

## Operate from CLI / TUI (UX)

1. CLI assurance-first workflow (+ h2kvm convert).
2. Convert if needed: `guestkit convert` / `qemu-img`.
3. `doctor --target … --explain`.
4. `migrate-plan --export`.
5. `repair --fix boot` / `migrate-repair --apply`.
6. Windows VirtIO via `GUESTKIT_VIRTIO_WIN`.
7. `passport emit/verify` then convert/boot on target.
8. **Empty / fail:** Boot fail post-cutover → fstab/GRUB/VirtIO; never write while guest running.
9. **Success:** Doctor high enough; passport verifies; first boot on target.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Migration Assurance](../assurance/migration-assurance.md)
- [Doctor](../assurance/doctor.md)
- [Migration Plan](../assurance/migrate-plan.md)
- [Repair](../fix-plans/repair.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
