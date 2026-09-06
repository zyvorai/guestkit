# FAQ

## Purpose

FAQ — Support surface.

## When to use it

- Operate **FAQ** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `faq`
- Nav: **Support → FAQ**
- Primary interface: Reference answers; commands embedded

## Operate from CLI / TUI (UX)

1. Reference answers; commands embedded.
2. Install: `cargo install guestkit`.
3. Not legacy appliance tooling — use GuestKit stack.
4. Passport vs virt-v2v: certify then convert.
5. Extract/list/rescue examples as in FAQ.
6. Cache under `~/.cache/guestkit/`; escalate with version + repro.
7. **Empty / fail:** N/A (doc); point to troubleshooting for runtime.
8. **Success:** Reader can run cited command successfully.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Troubleshooting](troubleshooting.md)
- [Getting Started](../onboarding/getting-started.md)
- [VM Migration Guide](../guides/vm-migration.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
