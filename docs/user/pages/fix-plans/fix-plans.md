# Fix Plans

## Purpose

Fix Plans — Fix Plans surface.

## When to use it

- Operate **Fix Plans** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `fix-plans`
- Nav: **Fix Plans → Fix Plans**
- Primary interface: `guestkit plan …` · export from migrate-plan/repair

## Operate from CLI / TUI (UX)

1. `guestkit plan …` · export from migrate-plan/repair.
2. `plan generate IMAGE -p linux-ssh|windows-rdp|… -o plan.yaml`.
3. `plan preview plan.yaml` (`--diff`).
4. `plan validate plan.yaml --vm IMAGE`.
5. `plan apply plan.yaml --vm IMAGE --yes`.
6. `plan export` to bash/ansible; `plan rollback` if needed.
7. **Empty / fail:** Apply refused without backup unless `--skip-backup`.
8. **Success:** Preview lists ops; apply reports success; doctor improves.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Repair](repair.md)
- [Migration Plan](../assurance/migrate-plan.md)
- [Profiles](../profiles/profiles.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
