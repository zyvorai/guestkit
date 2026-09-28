# What you can do

> Part of the [GuestKit README](../README.md). Back to the [documentation map](documentation-map.md).

## Assure · plan · certify · launch

```bash
guestkit doctor vm.qcow2 --target proxmox --explain
guestkit migrate-plan vm.vmdk --target proxmox --export plan.yaml
guestkit passport emit vm.qcow2 --target kvm -o passport.json
guestkit passport verify passport.json --fail-below 80
guestkit-qemu run vm.qcow2 --min-boot-score 80 --qmp-socket /run/guestkit/vm.qmp
```

## Repair offline (no boot required)

```bash
guestkit plan generate disk.qcow2 -p linux-ssh --user ubuntu --key-file ~/.ssh/id_ed25519.pub
guestkit rescue disk.qcow2 -o enable-ssh
guestkit rescue disk.qcow2 -o fix-grub --force
guestkit rescue win.qcow2 -o reset-password --user Administrator --password '…'
guestkit plan apply plan.yaml --vm disk.qcow2 --yes     # backups + rollback
```

## Local VM lifecycle, disk tools, and cutover

```bash
guestkit vm define demo disk.qcow2 --memory-mb 4096 --vcpus 2
guestkit vm start demo && guestkit vm status demo
guestkit img check disk.qcow2 --repair
guestkit domain-disks /etc/libvirt/qemu/web01.xml
guestkit firstboot win.qcow2 --hostname web01 --run 'echo hi'
guestkit gate --image disk.qcow2 --fail-below 80 --rego policies/cutover.rego
guestkit sbom-diff before.spdx.json after.spdx.json --fail-on-drift
virtctl-guestkit guestfs -n ns pvc
```

- **`guestkit vm`** — local QEMU lifecycle (define/start/pause/resume/destroy) for a lab or single box; production run/network/TTL is FluxVM's job — [features/vm-runtime.md](features/vm-runtime.md)
- **`guestkit img` / `domain-disks` / `firstboot`** — qemu-img wrapper, libvirt/YAML domain disk parsing, virtio-win plan, first-boot gate — [user-guides/img-firstboot.md](user-guides/img-firstboot.md)
- **Cutover bundle** — `gate` + SELinux/sysprep/BitLocker prep + cloud cutover profiles/Rego policy checks — [user-guides/cutover-bundle.md](user-guides/cutover-bundle.md), [user-guides/cutover-prep.md](user-guides/cutover-prep.md)
- **Passport handoff / fleet quarantine** — hand a passport to an h2kvmctl job, quarantine a fleet — [user-guides/handoff-quarantine.md](user-guides/handoff-quarantine.md)
- **Rescue dry-run Action + `sbom-diff`** — forensic-diff SBOM attach, CI extras — [devops/10-rescue-sbom-ci.md](devops/10-rescue-sbom-ci.md)
- **`virtctl-guestkit guestfs`** — drop-in for `virtctl guestfs` on a PVC, backed by GuestKit not libguestfs — [features/virtctl-guestkit.md](features/virtctl-guestkit.md)

## Live control · platform · AI

- **In-guest agent** (Linux + Windows) over virtio-serial / QGA — inject offline, then `agent-proxy` / `agent-call`
- **`guestkit qga`** — drop-in for `virsh qemu-agent-command` (direct unix socket; no virsh by default) — [virsh-to-guestkit.md](user-guides/virsh-to-guestkit.md)
- **Optional AI** (`--features ai`) — read-only tool-calling over the offline evidence snapshot; MCP server via `--features mcp`
- **KubeVirt** boot-inspect hooks and Guest Control Fabric
- **Web console** + worker on GHCR · Helm under `deploy/helm/zyvor`
- **Python:** `pip install zyvor-guestkit` → `import guestkit` + `run_doctor` / `run_migrate_repair` (v1.1.0+)
