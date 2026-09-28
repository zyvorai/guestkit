# Who does what (users)

> Part of the [GuestKit README](../README.md). Back to the [documentation map](documentation-map.md).

# Who does what (users)

| You need… | Use |
|-----------|-----|
| Score / repair a disk **before** power-on | **This repo (GuestKit)** |
| Boot the qcow2, give it a network, SSH, TTL, pause/resume | **[FluxVM](https://github.com/zyvorai/fluxvm)** |
| Hypervisor → KVM convert + import | **[h2kvm](https://github.com/zyvorai/h2kvm)** |

GuestKit does **not** own production networking (TAP/bridge/netns/DHCP) or disposable
fleet lifecycle. That is FluxVM. Keep GuestKit focused on offline intelligence.

## End-to-end: certify → run → manage

```bash
# ── 1. Certify & repair (GuestKit) ─────────────────────────────
guestkit doctor disk.qcow2 --target kvm --explain
guestkit plan generate disk.qcow2 -p virtio-initramfs -o virtio.yaml
guestkit plan apply virtio.yaml --vm disk.qcow2 --yes   # as needed
guestkit gate --image disk.qcow2 --fail-below 80        # CI / cutover gate
guestkit passport emit disk.qcow2 --target kvm -o passport.json

# ── 2. Run & manage (FluxVM) ─────────────────────────────────
# Point FluxVM at the same (or repaired) qcow2 — see FluxVM README.
# Overlay keeps the base disk untouched; pick a network mode:
#
#   user     — lab SSH via hostfwd (simplest)
#   tap      — join existing bridge (LAN DHCP)
#   tap+netns— known guest IP + NAT (isolated)
#
#   fluxvm create --spec my-vm.json
#   fluxvm get <id>          # status + guest_ip when netns
#   fluxvm exec <id> -- uptime
#   fluxvm delete <id>
```

Docs: [VM lifecycle / suite split](features/vm-runtime.md) ·
[FluxVM](https://github.com/zyvorai/fluxvm) ·
[Passport handoff](user-guides/handoff-quarantine.md)

## Libvirt / virsh → suite map

| Old habit | Replacement |
|-----------|-------------|
| `virsh define` / `start` / `destroy` (host-local QEMU) | **[FluxVM](https://github.com/zyvorai/fluxvm)** `create` / `get` / `delete` |
| libvirt NAT / bridge DHCP / guest IP | FluxVM `user` / `tap`+bridge / `tap`+`netns` (`guest_ip`) |
| `virsh qemu-agent-command` | `guestkit qga` (or `fluxvm exec` with vsock agent) |
| “Will it boot?” by `virsh start` | `guestkit doctor` / `passport` / `gate` **before** FluxVM create |
| KubeVirt / OpenShift domains | `virtctl` / Machina (unchanged) |

Full map: [virsh-to-guestkit.md](user-guides/virsh-to-guestkit.md).
