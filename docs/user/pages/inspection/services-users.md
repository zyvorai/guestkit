# Services & Users

## Purpose

Services & Users — Inspection surface.

## When to use it

- Operate **Services & Users** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `services-users`
- Nav: **Inspection → Services & Users**
- Primary interface: `guestkit systemd-services IMAGE`; users via REPL/inspect; TUI

## Operate from CLI / TUI (UX)

1. `guestkit systemd-services IMAGE`; users via REPL/inspect; TUI.
2. `guestkit systemd-services disk.qcow2` (`--failed`, `--service UNIT`).
3. `guestkit systemd-journal` / `systemd-boot` as needed.
4. Users: interactive → `users`, or inspect User Accounts.
5. TUI → Services / Users tabs.
6. `inspect --include-services`.
7. **Empty / fail:** Non-systemd OS → sparse services; Windows users via registry inspect.
8. **Success:** Enabled units listed; users show uid/shell.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Inspect](inspect.md)
- [Interactive Mode](../onboarding/interactive-mode.md)
- [Doctor](../assurance/doctor.md)
- [Guest Agent](../guest-agent/guest-agent.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
