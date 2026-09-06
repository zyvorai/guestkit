# Profiles

## Purpose

Profiles — Profiles surface.

## When to use it

- Operate **Profiles** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `profiles`
- Nav: **Profiles → Profiles**
- Primary interface: `guestkit inspect IMAGE --profile NAME` · `plan generate -p …`

## Operate from CLI / TUI (UX)

1. `guestkit inspect IMAGE --profile NAME` · `plan generate -p …`.
2. Inspect profiles: security, migration, performance, compliance, hardening, windows-migration.
3. `-o json` / `--export html|markdown`.
4. Day-0: `plan generate` with windows-rdp / linux-ssh.
5. Automate over fleet with shell loop.
6. `--cache-refresh` when needed.
7. **Empty / fail:** Unknown profile name.
8. **Success:** Profile sections + risk_level / inventory.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Inspect](../inspection/inspect.md)
- [Fix Plans](../fix-plans/fix-plans.md)
- [Export Formats](../export/export-formats.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
