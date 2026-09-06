# Zeus VM Tools

## Purpose

Zeus VM Tools — Integration surface.

## When to use it

- Operate **Zeus VM Tools** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `zeus-vm-tools`
- Nav: **Integration → Zeus VM Tools**
- Primary interface: Package zyvor-vm-tools / zyvor-guest-agent; API `/api/v1/vmtools/*`

## Operate from CLI / TUI (UX)

1. Package zyvor-vm-tools / zyvor-guest-agent; API `/api/v1/vmtools/*`.
2. Build packages.
3. `POST .../vmtools/install?method=auto|cloud-init|qga|iso`.
4. Check `GET /api/v1/vmtools/coverage`.
5. Quiesce: `.../vmtools/quiesce` / unquiesce.
6. Exec: `.../vmtools/exec`; offline: `agent-inject`.
7. **Empty / fail:** Coverage 0 → QGA down / no network → use qga file bootstrap.
8. **Success:** Agent connected; coverage chip; freeze/thaw works.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Guest Agent](../guest-agent/guest-agent.md)
- [KubeVirt Integration](kubevirt-integration.md)
- [Guest Control Fabric](../guest-files/guest-control-fabric.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
