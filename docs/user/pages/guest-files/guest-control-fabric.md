# Guest Control Fabric

## Purpose

Guest Control Fabric — Guest Files surface.

## When to use it

- Operate **Guest Control Fabric** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `guest-control-fabric`
- Nav: **Guest Files → Guest Control Fabric**
- Primary interface: Web/API via zyvor-api KubeVirt guest routes

## Operate from CLI / TUI (UX)

1. Web/API via zyvor-api KubeVirt guest routes.
2. Deploy API+UI (compose/Helm).
3. `GET .../guest/status` → control state.
4. `GET .../guest/capabilities` → transport.
5. Live: `POST .../guest/doctor` or agent-proxy.
6. Airgap: `POST .../guest/install-agent`; halted VM: `POST .../guest/repair-plan`.
7. **Empty / fail:** `console_only`/`blind_vm` → install QGA/agent or stop VM for offline path.
8. **Success:** Envelope with transport, controlState, ok:true.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Guest Agent](../guest-agent/guest-agent.md)
- [KubeVirt Integration](../integration/kubevirt-integration.md)
- [Zeus VM Tools](../integration/zeus-vm-tools.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
