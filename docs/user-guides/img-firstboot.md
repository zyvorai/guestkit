# `img`, `domain-disks`, `virtio-win`, `firstboot`

Four commands that close the remaining “shell out to virsh / qemu-img / find the ISO” gaps.

## `guestkit img`

GuestKit-owned `qemu-img`. Binary path: `$GUESTKIT_QEMU_IMG` (default `qemu-img`).

```bash
guestkit img info disk.qcow2
guestkit img check disk.qcow2
guestkit img check disk.qcow2 --repair          # qemu-img check -r leaks
guestkit img snapshots disk.qcow2
guestkit img snapshot-create disk.qcow2 --name pre-cutover
guestkit img snapshot-apply disk.qcow2 --name pre-cutover
guestkit img snapshot-delete disk.qcow2 --name pre-cutover
guestkit img resize disk.qcow2 +10G
guestkit img rebase overlay.qcow2 --backing base.qcow2
guestkit img commit overlay.qcow2
```

`convert` stays `guestkit convert` (already shipped).

## `guestkit domain-disks`

Parse libvirt domain XML or KubeVirt VM/VMI YAML. Does not call libvirt.

```bash
guestkit domain-disks /etc/libvirt/qemu/web01.xml
guestkit domain-disks vm.yaml --files-only | while read disk; do
  guestkit doctor "$disk" --target kubevirt
done
```

## `guestkit virtio-win`

```bash
export GUESTKIT_VIRTIO_WIN=/usr/share/virtio-win
guestkit virtio-win list
guestkit virtio-win plan --image win.qcow2 --json
# then apply with the existing repair path:
guestkit migrate-repair win.qcow2 --target kvm --virtio-win "$GUESTKIT_VIRTIO_WIN" --apply
```

Critical set: viostor, vioscsi, NetKVM, vioserial/vioser, balloon, viorng.

## `guestkit firstboot`

One JSON artifact for the cutover gate: offline doctor + optional live QGA ping + virtio-win plan + domain disk inventory.

```bash
guestkit firstboot win.qcow2 \
  --target kvm \
  --domain win.xml \
  --virtio-win /usr/share/virtio-win \
  --socket /var/lib/libvirt/qemu/channel/target/win/org.qemu.guest_agent.0 \
  --fail-below 80 \
  -o firstboot.json
```

`ready` is false when:

- `--fail-below` is set and there is no score, or the score is below N, or blockers exist
- a virtio-win tree was found but critical drivers are missing
- `--socket` was given (or a socket was discovered) and `guest-ping` failed

## Serial console without grubby

Photon and many other guests do not ship `grubby`. A repair command that used to be `grubby --update-kernel=ALL --args=console=ttyS0,115200 console=tty0` no longer fails with exit 127.

When `grubby` is absent, GuestKit writes `console=ttyS0,115200 console=tty0` into the bootloader the guest already uses: grub2 (`/etc/default/grub` and `grub.cfg`), BLS / systemd-boot, syslinux, extlinux, or zipl. If `grubby` is installed, that binary is still used.
