# Packages

## Purpose

Packages — Inspection surface.

## When to use it

- Operate **Packages** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `packages`
- Nav: **Inspection → Packages**
- Primary interface: `guestkit packages|pkg IMAGE` · REPL · TUI Packages

## Operate from CLI / TUI (UX)

1. `guestkit packages|pkg IMAGE` · REPL · TUI Packages.
2. `guestkit packages disk.qcow2`.
3. `--filter nginx` / `--limit 50`.
4. `--json` for automation.
5. Or `inspect --include-packages`.
6. Offline install staging: `GUESTKIT_PACKAGE_CACHE` + `plan apply`.
7. **Empty / fail:** Empty = no package DB / Windows guest / mount failed.
8. **Success:** Manager + package rows.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Inspect](inspect.md)
- [Fix Plans](../fix-plans/fix-plans.md)
- [Profiles](../profiles/profiles.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
