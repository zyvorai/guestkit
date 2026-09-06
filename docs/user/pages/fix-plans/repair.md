# Repair

## Purpose

Repair — Fix Plans surface.

## When to use it

- Operate **Repair** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `repair`
- Nav: **Fix Plans → Repair**
- Primary interface: `guestkit repair` · `rescue` · `migrate-repair`

## Operate from CLI / TUI (UX)

1. `guestkit repair` · `rescue` · `migrate-repair`.
2. `repair IMAGE --fix boot --dry-run`.
3. `repair IMAGE --fix boot` (apply + re-doctor).
4. Day-0: `rescue IMAGE -o enable-ssh|fix-grub|reset-password…`.
5. Migration: `migrate-repair IMAGE --target kvm [--apply --yes]`.
6. Prefer dry-run → backup → apply.
7. **Empty / fail:** Dry-run only = no disk change; NTFS dirty → ntfsfix first.
8. **Success:** Doctor score rises; rescue op applied.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Doctor](../assurance/doctor.md)
- [Fix Plans](fix-plans.md)
- [Guest Agent](../guest-agent/guest-agent.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
