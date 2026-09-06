# GuestKit VM lifecycle (`guestkit vm`)

GuestKit-native **lab / CI** QEMU helpers — a deliberately small virsh/libvirt
replacement for host-local smoke boots. GuestKit owns inspection and boot
assurance; QEMU owns execution; QMP owns day-2 power/pause ops.

There is no libvirt XML and no virsh dependency on this path.

## Suite goal (Zyvor)

| Product | Owns |
|---------|------|
| **GuestKit** | Certify + repair the disk: doctor, passport, gate, offline plans (SELinux, cloud-init, virtio, sysprep, …) |
| **[FluxVM](https://github.com/zyvorai/fluxvm)** | Run + manage the VM: QEMU/CH/Firecracker, TAP/netns/DHCP, cloud-init seed, TTL, pause/resume, fleets/CRD |

**Do not** grow GuestKit into a second disposable-compute plane. Host networking
(create TAP/bridge, netns+dnsmasq, known guest IP) and production lifecycle
belong in FluxVM. GuestKit may keep minimal `guestkit vm` / `guestkit-qemu`
for convert-and-boot smoke tests with **user-mode** (or a pre-created TAP).

Recommended pipeline:

```text
guestkit doctor / plan apply / passport emit
        │
        ▼  (passport verify passes)
fluxvm create --spec …   # qcow2 + network + cloud-init + TTL
```

KubeVirt / OpenShift domain objects still stay with `virtctl` / Machina.
Use `guestkit vm` only when you need a host-local QEMU smoke boot without
FluxVM installed.

## Commands

```bash
export GUESTKIT_VM_DIR=/var/lib/guestkit/vms
export GUESTKIT_VM_RUN_DIR=/run/guestkit/vms

guestkit vm define demo /path/to/demo.qcow2 --memory-mb 4096 --vcpus 2
guestkit vm plan demo
guestkit vm list
guestkit vm start demo          # may need --force + UEFI for converted disks
guestkit vm status demo
guestkit vm shutdown demo
guestkit vm reboot demo
guestkit vm pause demo
guestkit vm resume demo
guestkit vm destroy demo
guestkit vm undefine demo
```

## Assurance gate

`define` / `plan` / `start` re-run GuestKit doctor against the image.
`start` refuses by default when the boot score is below `--min-boot-score`
(or when blockers are present). Pass `--force` to override.

For user cutover, prefer raising the score with offline plans and emitting
a passport — then hand off to FluxVM — instead of relying on `--force`.

## Networking (intentionally minimal)

Default is QEMU **user-mode** networking. Optional `--ssh-port` forwards
`127.0.0.1:PORT → guest:22` only (loopback). `--tap IFACE` uses a
**pre-created** host TAP.

GuestKit does **not** create bridges, TAP devices, netns, or DHCP servers.
For LAN DHCP, known guest IPs, macvtap, or netns isolation, use **FluxVM**
(`network.mode: tap|user|macvtap`, optional `netns: true`).

## UEFI

Pass host firmware explicitly:

```bash
guestkit vm define uefi-demo disk.qcow2 \
  --uefi-code /usr/share/OVMF/OVMF_CODE_4M.fd \
  --uefi-vars /var/lib/guestkit/vms/uefi-demo_VARS.fd
```

## Relation to `guestkit-qemu` and FluxVM

| Tool | Role |
|------|------|
| `guestkit doctor` / `passport` / `plan` | Certify and repair |
| `guestkit-qemu plan\|run` | One-shot assurance → QEMU argv (lab) |
| `guestkit vm` | Named lab definitions + QMP lifecycle |
| **FluxVM** | Production/disposable run: overlay, network, cloud-init, TTL |

Prefer FluxVM for any user-facing “boot and manage this qcow2” (libvirt
replacement: create/list/get/delete + network/IP). Prefer `guestkit vm` /
`guestkit-qemu` only for assurance smoke without the FluxVM daemon.

## See also

- [QEMU / VirtIO runtime](qemu-runtime.md)
- [Dump virsh](../user-guides/virsh-to-guestkit.md)
- [Handoff / quarantine](../user-guides/handoff-quarantine.md)
- [FluxVM](https://github.com/zyvorai/fluxvm) — create/run/network/TTL
