# Migration Assurance

## Purpose

Migration Assurance — Assurance surface.

## When to use it

- Operate **Migration Assurance** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `migration-assurance`
- Nav: **Assurance → Migration Assurance**
- Primary interface: doctor → plan → policy → passport → fleet

## Operate from CLI / TUI (UX)

1. doctor → plan → policy → passport → fleet.
2. `doctor --explain`.
3. `migrate-plan --export`.
4. `inspect --profile windows-migration` if Windows.
5. `policy check`.
6. `passport emit … -o passport.json` then `passport verify --fail-below 80`.
7. Fleet: `fleet analyze` / `wave-plan`.
8. **Empty / fail:** Verify fails below threshold → repair then re-emit; BitLocker hard-block.
9. **Success:** Passport verify exit 0; doctor score acceptable for target.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Doctor](doctor.md)
- [Migration Plan](migrate-plan.md)
- [Policy Gate](policy.md)
- [Fleet](../fleet/fleet.md)
- [VM Migration Guide](../guides/vm-migration.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
