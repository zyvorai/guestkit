# Policy Gate

## Purpose

Policy Gate — Assurance surface.

## When to use it

- Operate **Policy Gate** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `policy`
- Nav: **Assurance → Policy Gate**
- Primary interface: `guestkit policy check IMAGE`

## Operate from CLI / TUI (UX)

1. `guestkit policy check IMAGE`.
2. `policy check IMAGE --example-policy`.
3. `policy check IMAGE --policy FILE.yaml`.
4. Or `--benchmark cis`.
5. `-f json -o report.json`.
6. Gate pipelines on non-zero / `--strict`; pair with passport verify.
7. **Empty / fail:** Missing policy file; expressions fail if evidence fields absent.
8. **Success:** Pass/fail per rule; JSON report written.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Doctor](doctor.md)
- [Migration Assurance](migration-assurance.md)
- [Fleet](../fleet/fleet.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
