# KubeVirt Integration

## Purpose

KubeVirt Integration — Integration surface.

## When to use it

- Operate **KubeVirt Integration** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `kubevirt-integration`
- Nav: **Integration → KubeVirt Integration**
- Primary interface: zyvor-api HTTP + CLI offline on PVC path

## Operate from CLI / TUI (UX)

1. zyvor-api HTTP + CLI offline on PVC path.
2. Deploy API with KubeVirt RBAC.
3. Stopped VM: `GET/POST /api/v1/kubevirt/vms/{ns}/{name}/boot-inspect`.
4. Or CLI: `doctor PVC_PATH --target kubevirt`.
5. Live: guest/status, guest/doctor, guest/evidence.
6. Install agent via guest/install-agent or vmtools.
7. **Empty / fail:** Running VM → offline inspect skipped; missing PVC path.
8. **Success:** available:true, source:guestkit, boot fields populated.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Guest Control Fabric](../guest-files/guest-control-fabric.md)
- [Guest Agent](../guest-agent/guest-agent.md)
- [Zeus VM Tools](zeus-vm-tools.md)
- [Doctor](../assurance/doctor.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
