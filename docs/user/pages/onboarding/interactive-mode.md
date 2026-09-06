# Interactive Mode

## Purpose

Interactive Mode — Onboarding surface.

## When to use it

- Operate **Interactive Mode** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `interactive-mode`
- Nav: **Onboarding → Interactive Mode**
- Primary interface: `guestkit interactive|repl|shell IMAGE`

## Operate from CLI / TUI (UX)

1. `guestkit interactive|repl|shell IMAGE`.
2. Launch REPL (one mount).
3. `info` → `filesystems` → `mount /dev/… /`.
4. `ls` / `cat` / `find` / `download`.
5. `packages` / `services` / `users` / `network`.
6. Optional `explore`; then `exit`.
7. **Empty / fail:** `ls /etc` empty usually means unmounted — run `mount`; launch fail → NBD/loop/permissions.
8. **Success:** Welcome shows detected OS; packages/users return lists.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Guest Files](../guest-files/files.md)
- [TUI (guestctl)](../interfaces/tui.md)
- [Inspect](../inspection/inspect.md)
- [Packages](../inspection/packages.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
