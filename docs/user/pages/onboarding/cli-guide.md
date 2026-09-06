# CLI Guide

## Purpose

CLI Guide — Onboarding surface.

## When to use it

- Operate **CLI Guide** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `cli-guide`
- Nav: **Onboarding → CLI Guide**
- Primary interface: `guestkit --help`; `guestkit commands`

## Operate from CLI / TUI (UX)

1. `guestkit --help`; `guestkit commands`.
2. `guestkit commands` for catalog.
3. Inspect group: inspect, filesystems, packages, network.
4. Assurance: doctor, migrate-plan, passport, policy, fleet.
5. Plans: `plan generate|preview|apply|rollback`.
6. Rescue: `rescue -o …`; prefer `-o json` / `--fail-below` for CI.
7. **Empty / fail:** Unknown subcommand → `guestkit commands`; feature-gated cmds need `--features agent|ai|mcp`.
8. **Success:** Help lists groups; sample inspect returns text/JSON.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Quick Reference](quick-reference.md)
- [Interactive Mode](interactive-mode.md)
- [Export Formats](../export/export-formats.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
