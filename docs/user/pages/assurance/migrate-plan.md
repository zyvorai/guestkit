# Migration Plan

## Purpose

Migration Plan — Assurance surface.

## When to use it

- Operate **Migration Plan** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `migrate-plan`
- Nav: **Assurance → Migration Plan**
- Primary interface: `guestkit migrate-plan IMAGE --target …` · TUI `e`

## Operate from CLI / TUI (UX)

1. `guestkit migrate-plan IMAGE --target …` · TUI `e`.
2. `guestkit migrate-plan IMAGE --target proxmox`.
3. `--explain` / `-o json`.
4. `--export plan.yaml` for FixPlan.
5. Optional `--inject-agent`.
6. Follow with migrate-assess / migrate-repair; TUI: `t`/`p`/`e`.
7. **Empty / fail:** Missing `--target`; Windows VirtIO gaps need `GUESTKIT_VIRTIO_WIN`.
8. **Success:** Migration score + checklist; YAML plan if exported.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Doctor](doctor.md)
- [Fix Plans](../fix-plans/fix-plans.md)
- [VM Migration Guide](../guides/vm-migration.md)
- [Migration Assurance](migration-assurance.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
