# Guest Files

## Purpose

Guest Files — Guest Files surface.

## When to use it

- Operate **Guest Files** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `files`
- Nav: **Guest Files → Guest Files**
- Primary interface: `list|ls`, `cat`, `extract|get`, `search|find`, `explore`

## Operate from CLI / TUI (UX)

1. `list|ls`, `cat`, `extract|get`, `search|find`, `explore`.
2. `guestkit ls disk.qcow2 /etc`.
3. `guestkit cat disk.qcow2 /etc/fstab`.
4. `guestkit extract disk.qcow2 /etc/hostname ./hostname.txt`.
5. `guestkit find disk.qcow2 '*.conf'`.
6. Multi-step: `interactive` or `explore`; backup: `guestkit backup IMAGE -o out.tar.gz`.
7. **Empty / fail:** Path missing → wrong mount/root; permission → sudo/NBD.
8. **Success:** Listing/content/extracted file on host.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Interactive Mode](../onboarding/interactive-mode.md)
- [TUI (guestctl)](../interfaces/tui.md)
- [Filesystems](../inspection/filesystems.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
