# Fleet

## Purpose

Fleet — Fleet surface.

## When to use it

- Operate **Fleet** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `fleet`
- Nav: **Fleet → Fleet**
- Primary interface: `guestkit fleet analyze|wave-plan|watch DIR` · TUI `--fleet DIR`

## Operate from CLI / TUI (UX)

1. `guestkit fleet analyze|wave-plan|watch DIR` · TUI `--fleet DIR`.
2. `fleet analyze ./vms/ [--recursive -j 4]`.
3. `fleet wave-plan ./vms/ -o json`.
4. `fleet watch ./vms/` (first run = baseline).
5. Later: `watch --fail-on-drift` in cron.
6. TUI: `guestctl tui one.qcow2 --fleet ./vms/`.
7. **Empty / fail:** Empty dir / no disk formats.
8. **Success:** Clusters/snowflakes/blockers; waves; watch reports drift or clean.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Doctor](../assurance/doctor.md)
- [Forensic Diff](../forensics/forensic-diff.md)
- [Migration Assurance](../assurance/migration-assurance.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
