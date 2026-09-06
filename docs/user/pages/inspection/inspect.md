# Inspect

## Purpose

Inspect — Inspection surface.

## When to use it

- Operate **Inspect** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `inspect`
- Nav: **Inspection → Inspect**
- Primary interface: `guestkit inspect IMAGE` · TUI Overview · web Image Vault

## Operate from CLI / TUI (UX)

1. `guestkit inspect IMAGE` · TUI Overview · web Image Vault.
2. `guestkit inspect disk.qcow2`.
3. Add `-o json` or `--summary`.
4. Depth: `--depth quick|standard|deep`.
5. Flags: `--include-packages|--include-services|--include-network`.
6. Profile: `--profile security|migration|performance|windows-migration`.
7. Batch: `inspect-batch *.qcow2 -p 4`.
8. **Empty / fail:** No OS → encrypted/corrupt/empty disk; use filesystems + interactive mount.
9. **Success:** OS type/distro/hostname + sections (or JSON).

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Filesystems](filesystems.md)
- [Packages](packages.md)
- [Profiles](../profiles/profiles.md)
- [Export Formats](../export/export-formats.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
