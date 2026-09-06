# guestkit Quick Start Guide

## Project Overview

**guestkit** is a pure Rust library and CLI for offline VM intelligence and **migration assurance**. It does **not** use legacy appliance tooling — disk access is via GuestKit's own engine (loop/NBD, partition/filesystem parsers, assurance APIs). Features include:

- 🩺 **Doctor / migrate-plan** - Boot probability and hypervisor-aware migration scoring before cutover
- 🖥️ **TUI Assurance** - Same scoring in `guestctl tui` (Security group · `d`/`t`/`p`/`e` keys)
- ▶️ **Assured QEMU launch** - `guestkit-qemu` turns evidence into a gated QEMU/VirtIO runtime
- 🎯 **Killer Summary View** - See OS, version, architecture at a glance
- 🪟 **Windows Registry Parsing** - Full Windows version detection (incl. `windows-migration` profile)
- 🔄 **VM Migration Support** - Universal fstab/crypttab rewriter + fix plans
- 💾 **Smart LVM Cleanup** - Automatic volume group management
- 🔄 **Loop Device Primary** - Built-in support for RAW/IMG/ISO

Designed to work seamlessly with [hyper2kvm](https://github.com/ssahani/hyper2kvm) and VM migration workflows.

## Building

```bash
cd ~/tt/guestkit

# Build the project
cargo build

# Build optimized release version
cargo build --release

# Run tests
cargo test
```

## Using the CLI

```bash
# Build and run
cargo run -- --help

# Convert VMDK to qcow2
cargo run -- convert \
  --source /path/to/vm.vmdk \
  --output /path/to/vm.qcow2 \
  --format qcow2 \
  --compress

# Detect disk format
cargo run -- detect --image /path/to/disk.img

# Get disk information
cargo run -- info --image /path/to/disk.img

# Verbose logging
cargo run -- -v convert --source vm.vmdk --output vm.qcow2
```

## Using as a Library

### In Your Cargo.toml

```toml
[dependencies]
guestkit = { path = "~/tt/guestkit" }
```

### Example Code

```rust
use guestkit::converters::DiskConverter;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let converter = DiskConverter::new();

    let result = converter.convert(
        Path::new("/path/to/source.vmdk"),
        Path::new("/path/to/output.qcow2"),
        "qcow2",
        true,  // compress
        true,  // flatten
    )?;

    if result.success {
        println!("✓ Conversion successful!");
        println!("  Source:  {} ({})",
            result.source_path.display(),
            result.source_format.as_str()
        );
        println!("  Output:  {} ({})",
            result.output_path.display(),
            result.output_format.as_str()
        );
        println!("  Size:    {} bytes", result.output_size);
        println!("  Time:    {:.2}s", result.duration_secs);
    }

    Ok(())
}
```

## Running Examples

```bash
# Convert disk
cargo run --example convert_disk

# Detect format
cargo run --example detect_format

# Retry example
cargo run --example retry_example
```

## TUI (interactive dashboard)

```bash
# Carbon-themed multi-view inspector
guestctl tui vm.qcow2

# Fleet of images
guestctl tui vm.qcow2 --fleet ./images/

# Compare second disk on dashboard
guestctl tui vm.qcow2 --compare other.qcow2
```

**Assurance** (Security group): offline `doctor` + `migrate-plan` parity with CLI — `d` run doctor, `t` cycle target (kvm/proxmox/aws), `p` preview fix plan, `e` export YAML. Dashboard **`a`** jumps to Assurance.

See [TUI enhancements](../features/tui-enhancements.md) and [migration assurance](../features/migration-assurance.md).

## Assured QEMU launch

After doctor/migrate-plan, launch under the same assurance gate:

```bash
cargo run --bin guestkit-qemu -- plan vm.qcow2 --json
cargo run --bin guestkit-qemu -- run vm.qcow2 --min-boot-score 80 \
  --qmp-socket /tmp/vm.qmp
```

UEFI guests need explicit firmware paths (`--uefi-code` / `--uefi-vars`).
Full guide: [qemu-runtime.md](../features/qemu-runtime.md).

## Live QGA (no virsh)

```bash
# Requires --features agent on Unix builds
cargo run --features agent -- qga --execute guest-ping
cargo run --features agent -- agent-call --method guestkit.getVersion
```

Cut-over map from `virsh qemu-agent-command`:
[virsh-to-guestkit.md](virsh-to-guestkit.md).

## Web console login

Packaged/remote installs ship a web console with a seeded default administrator:

| Username | Password |
|----------|----------|
| `admin`  | `Admin@321` |

> ⚠️ Change the password (and API key / `JWT_SECRET`) right after first login, and
> enable SSO/SAML from **Settings** before exposing the console beyond localhost.
> See the [remote deployment guide](../guides/DEPLOY-REMOTE.md#web-console-access).

Run the whole web stack from prebuilt GHCR images (no build):

```bash
docker compose -f deploy/docker-compose.ghcr.yml up -d   # → http://localhost:8088
```

The OSS Image Vault UI renders **inspect inventory tabs** (Summary → Security), **Assurance** (doctor / plan / passport / repair preview+apply), **Profiles**, and **Files** browse — see [Using the Dashboard](../customer/using-the-dashboard.md).

Lab HTTPS UI (static `deploy/ui` + built-in TLS, optional API proxy):

```bash
./scripts/deploy-ui-remote.sh <host> <user> --port 27173 \
  --api-upstream http://127.0.0.1:8080
```

See [Docker → Published images](../guides/DOCKER.md#published-images-ghcr) for tags, Helm, and auth options.

## Integration with h2kvm

h2kvm (formerly hyper2kvm) uses GuestKit as its default offline inspect/repair backend since v1.1.0.

### Python (recommended)

```bash
pip install "hypersdk-guestkit>=1.1.0"
```

```python
import guestkit

guestkit.run_doctor("source.vmdk", target="kvm", explain=True)
guestkit.run_migrate_repair("out.qcow2", target="kvm", apply=True)
```

h2kvm wraps the same calls in `h2kvm.core.guestkit_client`.

### CLI handoff

```bash
guestkit doctor disk.qcow2 --target kvm --explain
guestkit migrate-repair disk.qcow2 --target kvm --apply
h2kvmctl local --vmdk source.vmdk --to-output out.qcow2 --backend guestkit --libvirt-import
```

Full guide: [hyper2kvm-integration.md](../features/hyper2kvm-integration.md) · [h2kvm GUESTKIT.md](https://github.com/zyvorai/h2kvm/blob/main/docs/architecture/GUESTKIT.md)

## Development

### Project Structure

```
guestkit/
├── Cargo.toml          # Project configuration
├── src/
│   ├── lib.rs          # Library entry point
│   ├── main.rs         # CLI entry point
│   ├── core/           # Core utilities
│   ├── converters/     # Disk converters
│   └── ...
├── examples/           # Example programs
└── tests/              # Tests
```

### Adding New Features

1. **Create new module** in `src/`
2. **Export in lib.rs**
3. **Add tests**
4. **Update documentation**

### Running Tests

```bash
# All tests
cargo test

# Specific test
cargo test test_disk_format_conversion

# With logging
RUST_LOG=debug cargo test -- --nocapture
```

## Next Steps

1. **Assurance workflow** — `guestkit doctor` → `migrate-plan` → `migrate-repair`
2. **Python automation** — `pip install hypersdk-guestkit`; see [python-bindings.md](python-bindings.md)
3. **h2kvm pipeline** — [hyper2kvm-integration.md](../features/hyper2kvm-integration.md)
4. **CI gate** — GitHub Action + Passport verify
5. **Fleet ops** — `guestkit fleet analyze` / `watch`

## Troubleshooting

### Build Errors

```bash
# Update dependencies
cargo update

# Clean and rebuild
cargo clean && cargo build
```

### Missing qemu-img

```bash
# Fedora/RHEL
sudo dnf install qemu-img

# Ubuntu/Debian
sudo apt install qemu-utils
```

## Resources

- **README.md** - Comprehensive project documentation
- **examples/** - Working code examples
- **Cargo.toml** - Dependencies and configuration
- **hyper2kvm** - Primary integration target

## License

Apache-2.0
