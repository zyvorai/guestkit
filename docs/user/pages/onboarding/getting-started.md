# Getting Started

## Purpose

Getting Started — Onboarding surface.

## When to use it

- Operate **Getting Started** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `getting-started`
- Nav: **Onboarding → Getting Started**
- Primary interface: CLI first-hour; optional web `:8088`

## Operate from CLI / TUI (UX)

1. CLI first-hour; optional web `:8088`.
2. Install: `cargo install guestkit` or release binary.
3. Confirm deps: `qemu-img --version`.
4. `guestkit doctor vm.qcow2 --target kvm --explain`.
5. `guestkit migrate-plan vm.vmdk --target kvm --export plan.yaml`.
6. Optional: `guestctl tui vm.qcow2` or GHCR compose → `:8088`.
7. **Empty / fail:** Missing qemu-img/NBD → install qemu-utils; permission denied → sudo or `modprobe nbd`.
8. **Success:** Doctor prints 0–100 score + blockers; `guestkit version` works.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Quick Reference](quick-reference.md)
- [CLI Guide](cli-guide.md)
- [Doctor](../assurance/doctor.md)
- [TUI (guestctl)](../interfaces/tui.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
