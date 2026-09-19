# GuestKit integration for h2kvm

This directory contains **legacy and reference** integration utilities. For new projects, install the native Python module:

```bash
pip install zyvor-guestkit
```

## Recommended integration (v1.1.0+)

h2kvm delegates offline repair to GuestKit via PyO3 — **no subprocess wrapper required**:

```python
import json
import guestkit

# Assurance before convert
report = guestkit.run_doctor("source.vmdk", target="kvm", explain=True)

# Apply offline fixes during migration. inject_json is optional;
# omit it and repair is unchanged. Schema: docs/user-guides/python-bindings.md
result = guestkit.run_migrate_repair(
    "/var/lib/h2kvm/demo/ubuntu-test/ubuntu-test.qcow2",
    target="kvm",
    apply=True,
    inject_json=json.dumps({"hostname": "ubuntu-test"}),
)
```

h2kvm uses the same calls through `h2kvm.core.guestkit_client`. See [hyper2kvm-integration.md](../docs/features/hyper2kvm-integration.md).

## Integration options

| Approach | When to use |
|----------|-------------|
| **`pip install zyvor-guestkit`** + `run_*` APIs | **Default** — h2kvm, CI, automation |
| **GuestKit CLI** subprocess | Shell scripts, Passport CI gate, no Python |
| **`guestkit_wrapper.py`** (this dir) | Legacy hyper2kvm code paths only |
| **Direct Rust / GitHub Release v1.2.4** | Ops workstations, TUI, fleet tools |

### Option 1: Native Python (recommended)

```python
import guestkit

plan = guestkit.run_migrate_plan("vm.vmdk", target="kvm", export_fix_plan=True)
guestkit.run_migrate_repair("vm.qcow2", target="kvm", apply=True)
```

### Option 2: CLI subprocess

```python
import subprocess

subprocess.run([
    "guestkit", "migrate-repair", "vm.qcow2",
    "--target", "kvm", "--apply",
], check=True)
```

### Option 3: Legacy wrapper (subprocess to `guestkit convert`)

```python
from guestkit_wrapper import GuestkitWrapper

wrapper = GuestkitWrapper(guestkit_path="guestkit")
result = wrapper.convert(
    source_path="/path/to/vm.vmdk",
    output_path="/path/to/vm.qcow2",
    compress=True,
)
```

Prefer `import guestkit` for new code. The wrapper remains for backward compatibility.

## Deploy both to a migration host

```bash
# GuestKit CLI
GUESTKIT_ZYVOR_ACCEPT=1 ./scripts/deploy-remote.sh HOST user --quick --key

# h2kvm (pip installs zyvor-guestkit from PyPI)
cd /path/to/h2kvm
./scripts/deploy-remote.sh HOST user --keep-sources
```

## Testing

```bash
cargo build --release
cargo test

# Python bindings
pip install maturin
maturin develop --features python-bindings
python3 -c "import guestkit; print(guestkit.__version__)"

# Legacy wrapper smoke test
cd integration/python
python3 guestkit_wrapper.py
```

## Files

```
integration/
├── README.md                   # This file
├── python/
│   └── guestkit_wrapper.py     # Legacy subprocess wrapper
└── tests/
    └── test_integration.py
```

## See also

- [docs/features/hyper2kvm-integration.md](../docs/features/hyper2kvm-integration.md)
- [docs/user-guides/python-bindings.md](../docs/user-guides/python-bindings.md)
- [h2kvm deploy-remote](https://github.com/zyvorai/h2kvm/blob/main/docs/deployment/deploy-remote.md)
