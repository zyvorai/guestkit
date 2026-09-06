# Doctor

## Purpose

Doctor — Assurance surface.

## When to use it

- Operate **Doctor** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `doctor`
- Nav: **Assurance → Doctor**
- Primary interface: `guestkit doctor IMAGE` · TUI Assurance `d` · web Passport

## Operate from CLI / TUI (UX)

1. `guestkit doctor IMAGE` · TUI Assurance `d` · web Passport.
2. `guestkit doctor vm.qcow2 --target kvm|proxmox|kubevirt|aws…`.
3. `--explain` for root-cause.
4. `-o json --fail-below 80` for CI.
5. TUI: Assurance or `: doctor`.
6. Fix blockers → re-run doctor.
7. **Empty / fail:** Low score + blockers until repaired; mount failures abort.
8. **Success:** Score 0–100; exit 0 if above `--fail-below`.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Migration Plan](migrate-plan.md)
- [Repair](../fix-plans/repair.md)
- [Migration Assurance](migration-assurance.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
