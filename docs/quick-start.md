# 60-second quick start

> Part of the [GuestKit README](../README.md). Back to the [documentation map](documentation-map.md).

```bash
# v1.2.4 GitHub Release — crates.io `guestkit` is still 0.3.2
curl -fsSL -O https://github.com/zyvorai/guestkit/releases/download/v1.2.4/guestkit-1.2.4-linux-amd64.tar.gz

guestkit doctor vm.qcow2 --target proxmox --explain
guestkit migrate-plan vm.vmdk --target kvm --export plan.yaml
guestkit passport emit vm.qcow2 --target kvm -o passport.json
guestctl tui vm.qcow2           # Assurance · preview · export
guestkit-qemu plan vm.qcow2 --json   # assurance → QEMU definition

# Shrink an oversized-but-mostly-empty disk to its real footprint before import
guestkit shrink disk.qcow2 --dry-run                     # report only
guestkit shrink disk.qcow2 --min-ratio 3 --headroom-pct 20
```

**CI gate** — same score, no CLI install step:

```yaml
- uses: zyvorai/guestkit@v1
  with:
    disk: vm.qcow2
    target: kvm
    fail-below: '80'
```

Targets: `kvm` · `proxmox` · `qemu` · `kubevirt` · `aws` · `azure` · `gcp` · `hyperv`

Host needs: Linux with `qemu-img`, `losetup`, and `qemu-nbd` (mount/repair may need root).

## Python (v1.1.0+)

Same assurance engine as the CLI — on **[PyPI](https://pypi.org/project/zyvor-guestkit/)** and used by **h2kvm** offline fixer:

```bash
pip install zyvor-guestkit
```

```python
import guestkit

guestkit.run_doctor("vm.qcow2", target="kvm", explain=True)
guestkit.run_migrate_repair("vm.qcow2", target="kvm", apply=False)  # dry-run
guestkit.run_migrate_repair("vm.qcow2", target="kvm", apply=True)   # apply fixes
# Optional inject_json= (hostname, network, users, first-boot). CLI has no inject flag.
```

See [python-bindings.md](user-guides/python-bindings.md) and [examples/python/assurance_doctor.py](../examples/python/assurance_doctor.py).

| You want… | Go here |
|-----------|---------|
| First hour | [Getting started](user-guides/getting-started.md) |
| Python assurance APIs | [python-bindings.md](user-guides/python-bindings.md) |
| **Assured QEMU launch** | [qemu-runtime.md](features/qemu-runtime.md) |
| **h2kvm pipeline** | [hyper2kvm-integration.md](features/hyper2kvm-integration.md) |
| Remote SSH deploy | [DEPLOY-REMOTE.md](guides/DEPLOY-REMOTE.md) |
| Cheat sheet | [Quick reference](user-guides/quick-reference.md) |
| Full feature map | [User feature guide](guestkit-user-feature-guide.md) |
| Open source vs Enterprise | [ce-vs-enterprise.md](ce-vs-enterprise.md) |
