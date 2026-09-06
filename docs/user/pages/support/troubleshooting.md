# Troubleshooting

## Purpose

Troubleshooting — Support surface.

## When to use it

- Operate **Troubleshooting** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `troubleshooting`
- Nav: **Support → Troubleshooting**
- Primary interface: Ops runbook — CLI diagnostics

## Operate from CLI / TUI (UX)

1. Ops runbook — CLI diagnostics.
2. `guestkit -v inspect …` / `RUST_LOG=debug`.
3. NBD: `modprobe nbd`; `qemu-nbd --disconnect`.
4. Loop: `losetup -D`.
5. `qemu-img check IMAGE`.
6. Container: privileged + `/dev/nbd*`; gather version + uname + lsmod.
7. **Empty / fail:** Persistent device busy → disconnect all nbd / reboot.
8. **Success:** Same failing command succeeds after fix.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [FAQ](faq.md)
- [Getting Started](../onboarding/getting-started.md)
- [Filesystems](../inspection/filesystems.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
