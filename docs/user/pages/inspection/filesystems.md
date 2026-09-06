# Filesystems

## Purpose

Filesystems — Inspection surface.

## When to use it

- Operate **Filesystems** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `filesystems`
- Nav: **Inspection → Filesystems**
- Primary interface: `guestkit filesystems|fs IMAGE` · REPL · TUI Storage

## Operate from CLI / TUI (UX)

1. `guestkit filesystems|fs IMAGE` · REPL · TUI Storage.
2. `guestkit filesystems disk.qcow2`.
3. `--detailed` for types/labels.
4. Cross-check `guestkit usage|df` and `check|fsck`.
5. In REPL: `mount` root then `ls /`.
6. LVM clues via `inspect --profile migration`.
7. **Empty / fail:** Empty partition list → wrong image / need NBD; LUKS needs cryptsetup.
8. **Success:** Partition/FS list (ext4/xfs + swap).

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Inspect](inspect.md)
- [Guest Files](../guest-files/files.md)
- [Repair](../fix-plans/repair.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
