# Forensic Diff

## Purpose

Forensic Diff — Forensics surface.

## When to use it

- Operate **Forensic Diff** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `forensic-diff`
- Nav: **Forensics → Forensic Diff**
- Primary interface: `guestkit forensic-diff OLD NEW` · `diff` / `compare`

## Operate from CLI / TUI (UX)

1. `guestkit forensic-diff OLD NEW` · `diff` / `compare`.
2. Snapshot before/after.
3. `forensic-diff before.qcow2 after.qcow2`.
4. `-o json` for drift score.
5. Lighter: `guestkit diff a b`.
6. Fleet continuous: `fleet watch`.
7. **Empty / fail:** Identical images → low drift; mount fail on either side.
8. **Success:** Drift findings / security indicators JSON.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Fleet](../fleet/fleet.md)
- [Inspect](../inspection/inspect.md)
- [Profiles](../profiles/profiles.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
