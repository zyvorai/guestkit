# Network Inspect

## Purpose

Network Inspect — Inspection surface.

## When to use it

- Operate **Network Inspect** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `network`
- Nav: **Inspection → Network Inspect**
- Primary interface: `guestkit network IMAGE` · REPL · TUI Network

## Operate from CLI / TUI (UX)

1. `guestkit network IMAGE` · REPL · TUI Network.
2. `guestkit network disk.qcow2`.
3. `--show-interfaces` / `--show-dns` / `--show-routes`.
4. `--export-json`.
5. Or `inspect --include-network`.
6. Live: agent-proxy `/evidence` or KubeVirt guest/network.
7. **Empty / fail:** No ifaces → cloud-init-only / netplan not parsed.
8. **Success:** Interfaces + IP/MAC/DHCP + DNS.

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Inspect](inspect.md)
- [Migration Plan](../assurance/migrate-plan.md)
- [Guest Agent](../guest-agent/guest-agent.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
