# TUI (guestctl)

## Purpose

TUI (guestctl) — Interfaces surface.

## When to use it

- Operate **TUI (guestctl)** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `tui`
- Nav: **Interfaces → TUI (guestctl)**
- Primary interface: `guestctl tui IMAGE` · `guestkit tui|ui IMAGE`

## Operate from CLI / TUI (UX)

1. `guestctl tui IMAGE` · `guestkit tui|ui IMAGE`.
2. Open TUI (optional `--fleet DIR`, `--compare OTHER`).
3. `{`/`}` groups; Tab views; Ctrl+P jump; `:` palette.
4. Assurance: `d` doctor, `t` target, `p` preview, `e` export.
5. Browse Packages/Services/Users/Network.
6. Config `~/.config/guestkit/tui.toml`; `h`/`?` help.
7. **Empty / fail:** Blank panes → inspect failed (permissions/format); check footer.
8. **Success:** Dashboard populated; doctor score on Assurance.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Doctor](../assurance/doctor.md)
- [Migration Plan](../assurance/migrate-plan.md)
- [Interactive Mode](../onboarding/interactive-mode.md)
- [Inspect](../inspection/inspect.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
