# Guest Agent

## Purpose

Guest Agent — Guest Agent surface.

## When to use it

- Operate **Guest Agent** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `guest-agent`
- Nav: **Guest Agent → Guest Agent**
- Primary interface: Build `--features agent`; inject offline; proxy on host

## Operate from CLI / TUI (UX)

1. Build `--features agent`; inject offline; proxy on host.
2. `cargo build --release --features agent --target x86_64-unknown-linux-musl`.
3. `guestkit agent-inject IMAGE --agent-binary …`.
4. Boot guest; `guestkit agent-proxy --socket … --listen 127.0.0.1:8765`.
5. `curl /ping` `/doctor` `/evidence`.
6. Or `guestkit agent-call --socket … --method guestkit.getEvidence`.
7. Prefer **`guestkit qga --execute guest-ping`** over `virsh qemu-agent-command` ([virsh-to-guestkit.md](../../../user-guides/virsh-to-guestkit.md)).
8. **Empty / fail:** No channel → missing virtio-serial/QGA; Windows needs virtio-serial driver.
9. **Success:** `/ping` OK; RPC returns doctor JSON.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Guest Control Fabric](../guest-files/guest-control-fabric.md)
- [KubeVirt Integration](../integration/kubevirt-integration.md)
- [Repair](../fix-plans/repair.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
