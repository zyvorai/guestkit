# Quick Reference

## Purpose

Quick Reference — Onboarding surface.

## When to use it

- Operate **Quick Reference** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `quick-reference`
- Nav: **Onboarding → Quick Reference**
- Primary interface: Cheat sheet — CLI / TUI keys

## Operate from CLI / TUI (UX)

1. Cheat sheet — CLI / TUI keys.
2. `guestkit inspect disk.qcow2`.
3. `guestkit doctor IMAGE --target kvm`.
4. `guestkit migrate-plan IMAGE --target proxmox --export plan.yaml`.
5. `guestkit rescue IMAGE -o enable-ssh` / `fix-grub`.
6. `guestctl tui IMAGE` → `d`/`t`/`p`/`e`.
7. **Empty / fail:** No OS found → check filesystems; wrong format → detect/info.
8. **Success:** Inspect shows OS/hostname; doctor score present.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [CLI Guide](cli-guide.md)
- [Inspect](../inspection/inspect.md)
- [Doctor](../assurance/doctor.md)
- [TUI (guestctl)](../interfaces/tui.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
