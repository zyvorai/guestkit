# h2kvm integration

> Part of the [GuestKit README](../README.md). Back to the [documentation map](documentation-map.md).

GuestKit provides **offline disk intelligence**; [h2kvm](https://github.com/zyvorai/h2kvm) provides **hypervisor-to-KVM conversion and deploy**.

```text
  guestkit doctor / migrate-plan     ← pre-flight score + fix plan
              │
              ▼
  h2kvmctl local --backend guestkit  ← convert + run_migrate_repair
              │
              ▼
  libvirt · KubeVirt · OpenStack
```

```bash
# GuestKit from PyPI; h2kvm from GitHub Release
pip install zyvor-guestkit
pip install https://github.com/zyvorai/h2kvm/releases/download/v1.1.0/h2kvm-1.1.0-py3-none-any.whl

# Pre-flight
guestkit doctor source.vmdk --target kvm --explain

# Convert + offline repair
h2kvmctl local --vmdk source.vmdk --to-output out.qcow2 --backend guestkit
```

Deploy both to a lab host:

```bash
GUESTKIT_ZYVOR_ACCEPT=1 ./scripts/deploy-remote.sh HOST user --quick --key   # GuestKit CLI
cd /path/to/h2kvm && ./scripts/deploy-remote.sh HOST user --keep-sources      # h2kvm
```

Full guide: **[hyper2kvm-integration.md](features/hyper2kvm-integration.md)** · **[h2kvm README](https://github.com/zyvorai/h2kvm#readme)**
