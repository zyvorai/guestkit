# Frequently Asked Questions (FAQ)

Common questions and answers about guestkit.

## General Questions

### What is guestkit?

guestkit is a pure Rust toolkit for VM disk inspection and manipulation without booting the VM. It provides:
- Beautiful emoji-enhanced CLI output
- Complete OS detection (Linux & Windows)
- VM migration support (fstab/crypttab rewriter)
- Windows registry parsing
- Python bindings for automation
- 578 disk image manipulation functions

### How is GuestKit different from legacy appliance tooling?

**GuestKit does not use legacy appliance tooling.** It is a pure Rust alternative with its own disk, partition, and filesystem stack (`guestkit::guestfs`, loop/NBD, assurance APIs). You do not need `legacy guest tools`, or `virt-inspector` to run GuestKit.

The table below compares products for migration planning — not a dependency list:

| Feature | guestkit | legacy appliance tooling |
|---------|----------|----------------------------|
| **Language** | Pure Rust | C + bindings |
| **Dependencies** | Minimal (qemu-img, nbd for some formats) | Many C libraries |
| **Installation** | Single binary | Complex dependencies |
| **Performance** | Fast (loop devices) | Appliance boot overhead |
| **API Coverage** | GuestKit assurance + inspect APIs | Full virt-* ecosystem |
| **Windows Support** | Registry parsing built-in | Requires Windows tools |
| **Visual Output** | Beautiful emojis + colors | Plain text |
| **Migration** | Built-in fstab/crypttab rewriter | Manual scripting |

**Bottom line:** Use GuestKit APIs (`doctor`, `migrate-plan`, `passport`, `run_boot_inspect`, Zyvor HTTP routes). Do not install legacy appliance tooling expecting GuestKit to call it.

### How does GuestKit compare to Red Hat virt-v2v / MTV?

They solve different jobs:

| Role | Tool |
|------|------|
| Discover / export | **HyperSDK** (`hyperctl`) |
| Convert / deploy to KVM | **[h2kvm](https://github.com/zyvorai/h2kvm)** (`h2kvmctl`) or virt-v2v/MTV |
| Certify cutover (score → fix → re-score) | **GuestKit Cutover Passport** |

RH virt-v2v and MTV are **convert-first**. GuestKit is **assurance-first**: emit a reviewable Passport, gate CI with `passport verify --fail-below 80`, then hand off to h2kvm. MTV cannot skip that gate.

```bash
guestkit passport emit vm.qcow2 --target kvm -o passport.json
guestkit passport verify passport.json --fail-below 80
# then: h2kvmctl migrate … / HyperSDK job pipeline
```

### Why does guestkit require sudo?

guestkit requires elevated privileges for:
- Mounting loop devices (`losetup`)
- Loading NBD kernel module (`modprobe nbd`)
- Accessing `/dev/nbd*` devices
- Mounting filesystems

**Exception:** Read-only operations with cached results don't need sudo:
```bash
# First run needs sudo
sudo guestkit inspect vm.qcow2 --cache

# Subsequent runs from cache (no sudo!)
guestkit inspect vm.qcow2 --cache
```

### Is guestkit production-ready?

**Yes.** GuestKit v1.2.4 is production-ready with:
- Python assurance bindings (`run_doctor`, `run_migrate_repair`, optional `inject_json`) for h2kvm integration
- Comprehensive test suite with CI/CD
- Used in [h2kvm](https://github.com/zyvorai/h2kvm) for production VM migrations
- Pure Rust for memory safety
- Extensive documentation and examples

### Can I use guestkit on running VMs?

**Read-only: Yes (with caution)**
```bash
# Read-only inspection of running VM (risky but possible)
guestkit inspect running-vm.qcow2
```

**Read-write: NO!**
Never modify a disk image while the VM is running. This will cause:
- Data corruption
- Filesystem damage
- Potential data loss

**Best Practice:** Shut down VM before using guestkit for modifications.

## Installation & Setup

### How do I install guestkit?

**Current CLI (v1.2.4)** — GitHub Release. `cargo install guestkit` still installs crates.io **0.3.2**, which is not this release.

```bash
curl -fsSL -o guestkit-1.2.4-linux-amd64.tar.gz \
  https://github.com/zyvorai/guestkit/releases/download/v1.2.4/guestkit-1.2.4-linux-amd64.tar.gz
```

**From source:**
```bash
git clone https://github.com/zyvorai/guestkit
cd guestkit
cargo build --release
sudo cp target/release/guestkit target/release/guestctl \
  target/release/guestkit-qemu /usr/local/bin/
```

**Python bindings:**
```bash
pip install zyvor-guestkit
```

### What binaries does GuestKit install?

| Binary | Role |
|--------|------|
| `guestkit` | Scriptable CLI (inspect, doctor, migrate-plan, rescue, **`qga`**, …) |
| `guestctl` | Carbon TUI dashboard |
| `guestkit-qemu` | Assured QEMU plan / run / QMP ([qemu-runtime.md](../features/qemu-runtime.md)) |

### Should I use `virsh qemu-agent-command`?

No. Prefer **`guestkit qga`** (raw QGA) or **`guestkit agent-call`** (GuestKit
JSON-RPC) against the guest-agent unix socket. Domain lifecycle
(`list`/`start`/`destroy`) stays with `virtctl` / Machina — GuestKit is not a
libvirt replacement. See [virsh-to-guestkit.md](virsh-to-guestkit.md).
`GUESTKIT_ALLOW_VIRSH=1` is an emergency zyvor-api opt-in only.

### What are the system requirements?

**Operating System:**
- Linux (primary support)
- macOS (limited support, no KVM)
- Windows WSL2 (experimental)

**Dependencies:**
- **Required:** Rust 1.70+ (for building)
- **Optional:** qemu-img (for format conversion)
- **Runtime:** Linux kernel with loop device support (built-in)
- **QEMU launch:** `qemu-system-x86_64` / `qemu-system-aarch64` on `PATH` for `guestkit-qemu run`

**Hardware:**
- KVM support recommended for performance
- Minimum 2GB RAM
- 1GB free disk space

### Why does installation fail on Ubuntu 20.04?

**Issue:** Rust version too old

**Solution:**
```bash
# Update Rust to latest
rustup update

# Or install newer Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Then build the current release from source (crates.io is still 0.3.2)
git clone https://github.com/zyvorai/guestkit
cd guestkit && cargo build --release
```

## Usage Questions

### How do I inspect a QCOW2 file?

```bash
# Basic inspection
sudo guestkit inspect vm.qcow2

# With JSON output
sudo guestkit inspect vm.qcow2 --output json

# With caching for speed
sudo guestkit inspect vm.qcow2 --cache
```

### Can I inspect Windows VMs?

**Yes!** guestkit v0.3.1+ has full Windows support:

```bash
sudo guestkit inspect windows.qcow2
```

Output includes:
- Windows version (Windows 10, 11, Server)
- Windows edition (Home, Pro, Enterprise)
- Build number
- Hostname
- Installed applications
- Users


### How do I extract files from a VM?

```bash
# Extract single file
sudo guestkit extract vm.qcow2 /etc/hostname ./hostname.txt

# Or use interactive mode
sudo guestkit interactive vm.qcow2
> mount /
> download /etc/hostname ./hostname.txt
> exit

# Or use cat to view
sudo guestkit cat vm.qcow2 /etc/hostname
```

### How do I list files in a VM?

```bash
# List directory
sudo guestkit list vm.qcow2 /etc

# Or interactive mode
sudo guestkit interactive vm.qcow2
> mount /
> ls /etc
> exit
```

### How do I repair GRUB offline?

```bash
guestkit rescue linux.qcow2 -o check-grub          # diagnose
guestkit rescue linux.qcow2 -o fix-grub            # chroot mkconfig / first-boot fallback
guestkit rescue linux.qcow2 -o fix-grub --force    # also try grub-install on NBD
guestkit plan generate linux.qcow2 -p linux-grub --grub-timeout 5 -o grub.yaml
```

Details: [fix-plans.md](../features/fix-plans.md#rescue-shortcuts).

### How do I stage packages into an offline guest?

```bash
export GUESTKIT_PACKAGE_CACHE=~/rpm-cache
export GUESTKIT_PACKAGE_FETCH=1   # optional: dnf/apt-get download on the host
# optional on macOS / hosts without dnf|apt:
# export GUESTKIT_PACKAGE_MIRROR=https://mirror.example/pkgs
guestkit plan apply plan.yaml --vm linux.qcow2 --yes
```

Matching `.rpm`/`.deb` files are copied into the guest and a first-boot systemd oneshot installs them.

### Why is guestkit slow on QCOW2 files?

**Reason:** QCOW2 requires NBD (Network Block Device) which is slower than loop devices.

**Solutions:**
1. **Use caching:**
   ```bash
   sudo guestkit inspect vm.qcow2 --cache  # First run slow, subsequent fast
   ```

2. **Convert to RAW:**
   ```bash
   qemu-img convert -O raw vm.qcow2 vm.raw
   sudo guestkit inspect vm.raw  # Much faster (loop device)
   ```

3. **Use parallel processing:**
   ```bash
   sudo guestkit inspect-batch *.qcow2 --parallel 4
   ```

### How do I compare two VMs?

```bash
# Compare two VMs
sudo guestkit diff vm1.qcow2 vm2.qcow2

# Compare multiple VMs against baseline
sudo guestkit compare baseline.qcow2 vm1.qcow2 vm2.qcow2 vm3.qcow2
```

## Migration Questions

### How do I migrate from Hyper-V to KVM?

Complete workflow:

```bash
# 1. Convert VHDX to QCOW2
qemu-img convert -f vhdx -O qcow2 vm.vhdx vm.qcow2

# 2. Inspect and plan
sudo guestkit inspect vm.qcow2 --profile migration

# 3. Modify configuration (Linux VMs)
sudo guestkit interactive vm.qcow2
> mount /
> command "sed -i 's/\/dev\/sda/\/dev\/vda/g' /etc/fstab"
> exit

# 4. For Windows: Inject VirtIO drivers
virt-win-reg vm.qcow2 --merge virtio-drivers.reg

# 5. Test boot
virt-install --name test --disk vm.qcow2 --import
```

See [VM Migration Guide](vm-migration.md) for complete details.

### Can guestkit rewrite fstab automatically?

**Yes!** v0.3.1+ includes universal fstab/crypttab rewriter:

```rust
use guestkit::guestfs::Guestfs;
use std::collections::HashMap;

let mut g = Guestfs::new()?;
g.add_drive("vm.qcow2")?;
g.launch()?;

let roots = g.inspect_os()?;
let mut device_map = HashMap::new();
device_map.insert("/dev/sda", "/dev/vda");

g.rewrite_fstab(&roots[0], &device_map)?;
g.shutdown()?;
```

### How do I migrate encrypted VMs (LUKS)?

```bash
# Inspect encrypted VM
sudo guestkit inspect encrypted.qcow2

# Open LUKS volume
sudo guestkit interactive encrypted.qcow2
> luks-open /dev/sda2 luks-root <passphrase>
> mount /dev/mapper/luks-root /
> # Make modifications
> umount /
> luks-close luks-root
> exit
```

See [VM Migration Guide - Encrypted Volumes](vm-migration.md#encrypted-volume-migration).

## Windows-Specific Questions

### Does guestkit work with Windows VMs?

**Yes!** Full Windows support in v0.3.1+:
- Windows 7 through Windows 11
- Windows Server 2008 R2 through 2022
- Registry parsing for version detection
- User account listing
- Installed application detection
- VirtIO driver injection support

### How does Windows version detection work?

guestkit reads Windows registry hives directly from disk:

```rust
// Internal implementation (simplified)
1. Mount Windows system partition
2. Read C:\Windows\System32\config\SOFTWARE registry hive
3. Parse "HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion"
4. Extract ProductName, CurrentBuild, EditionID
5. Map to human-readable version (Windows 11 Pro, etc.)
```

### Can I modify Windows registry offline?

**Yes**, with a build that includes `--features registry-write` (libhivex). Fix-plan `RegistryEdit` ops and rescue Windows day-0 commands mutate SOFTWARE/SYSTEM/SAM/SECURITY hives with backup.

```bash
# Interactive modification
sudo guestkit interactive windows.qcow2
> mount C:
> registry-set "HKLM\\SYSTEM\\..." "KeyName" "Value"
> exit

# Or use virt-win-reg
virt-win-reg windows.qcow2 --merge custom.reg
```


### How do I reset a Windows local password offline?

```bash
# Prefer AES/RC4 SAM NT-hash write (SYSKEY); needs registry-write + SYSTEM hive
guestkit rescue win.qcow2 -o reset-password --user Administrator --password 'S3cret!'

# Blank only (chntpw-style) — omit --password
guestkit rescue win.qcow2 -o reset-password --user Administrator
```

If AES write fails, GuestKit falls back to SAM blank + first-boot RunOnce `net user`. Details: [fix-plans.md](../features/fix-plans.md#rescue-shortcuts).

### How do I inject VirtIO drivers for Windows?

**Before migration:**
```bash
# Create registry entries for VirtIO drivers
cat > virtio.reg <<EOF
[HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Services\vioscsi]
"Start"=dword:00000000
EOF

# Inject into registry
virt-win-reg windows.qcow2 --merge virtio.reg
```

**After first boot:**
- Attach virtio-win.iso to VM
- Install drivers from Device Manager


## Performance Questions

### Why is first run slow, subsequent runs fast?

**Caching!** guestkit caches inspection results:

```bash
# First run: ~30 seconds (full inspection)
sudo guestkit inspect vm.qcow2 --cache

# Second run: <0.5 seconds (from cache)
sudo guestkit inspect vm.qcow2 --cache

# Cache is invalidated if VM file changes
```

**Cache location:** `~/.cache/guestkit/`

### How can I speed up batch processing?

**Use parallel processing:**

```bash
# Inspect 8 VMs in parallel (4 workers)
sudo guestkit inspect-batch vm*.qcow2 --parallel 4 --cache

# With progress bar
sudo guestkit inspect-batch vm*.qcow2 --parallel 4 --cache --progress
```

**Performance:** 4x speedup with 4 workers, plus 60x from caching on subsequent runs.

### Should I use RAW or QCOW2 format?

| Criteria | Recommendation |
|----------|----------------|
| **Performance** | RAW (loop device, faster) |
| **Disk space** | QCOW2 (compressed, smaller) |
| **Snapshots** | QCOW2 (built-in snapshots) |
| **Simplicity** | RAW (no overhead) |
| **Development** | QCOW2 (snapshots useful) |
| **Production databases** | RAW (best I/O) |

**Best of both worlds:**
```bash
# Develop with QCOW2 (snapshots)
guestkit inspect dev.qcow2

# Deploy with RAW (performance)
qemu-img convert -O raw dev.qcow2 prod.raw
guestkit inspect prod.raw  # Fast!
```

## API & Development Questions

### Can I use guestkit from Python?

**Yes!** Full Python bindings:

```python
from guestkit import Guestfs

with Guestfs() as g:
    g.add_drive_ro("vm.qcow2")
    g.launch()

    roots = g.inspect_os()
    for root in roots:
        print(f"OS: {g.inspect_get_distro(root)}")
        print(f"Hostname: {g.inspect_get_hostname(root)}")
```

**Installation:**
```bash
pip install zyvor-guestkit
```

See [Python Bindings Guide](python-bindings.md).

### What API functions are available?

**578 functions across 95 modules including:**
- OS inspection (30+ functions)
- File operations (50+ functions)
- Partition management (20+ functions)
- Filesystem operations (40+ functions)
- LVM support (15+ functions)
- Archive operations (10+ functions)
- Windows registry (15+ functions)
- And many more...

### How do I contribute to guestkit?

```bash
# 1. Fork repository
gh repo fork zyvorai/guestkit

# 2. Clone your fork
git clone https://github.com/YOUR_USERNAME/guestkit
cd guestkit

# 3. Create feature branch
git checkout -b feature/my-enhancement

# 4. Make changes and test
cargo test
cargo clippy

# 5. Submit pull request
gh pr create --title "Add: My enhancement"
```

See [CONTRIBUTING.md](../development/CONTRIBUTING.md) for guidelines.

## Troubleshooting Questions

### "Failed to launch appliance" error?

**Causes:**
1. KVM not available
2. Insufficient permissions
3. Memory constraints

**Solutions:**
```bash
# 1. Check KVM
ls -la /dev/kvm
# If missing: Enable VT-x/AMD-V in BIOS

# 2. Check permissions
sudo usermod -a -G kvm $USER
# Logout and login for group to take effect

# 3. Check memory
free -h
# Need at least 1GB free

# 4. Slow inspect on QCOW2 — ensure NBD module and privileges
sudo modprobe nbd max_part=16
sudo guestkit inspect vm.qcow2 --cache
```

### "No OS detected" error?

**Common causes:**
1. Disk not bootable
2. Encrypted without passphrase
3. Unsupported OS
4. Corrupted filesystem

**Debug steps:**
```bash
# Check disk structure
sudo guestkit filesystems vm.qcow2

# Try interactive mode
sudo guestkit interactive vm.qcow2
> list-devices
> list-partitions
> mount /dev/sda1
> ls /
```

See [Troubleshooting Guide](troubleshooting.md).

### Why can't guestkit access NBD devices?

**Error:** "Could not load NBD module"

**Solution:**
```bash
# Load NBD module manually
sudo modprobe nbd max_part=8

# Make persistent
echo "nbd" | sudo tee /etc/modules-load.d/nbd.conf

# Alternative: Use RAW format (no NBD needed)
qemu-img convert -O raw vm.qcow2 vm.raw
```

### "Permission denied" even with sudo?

**SELinux issue:**

```bash
# Check SELinux status
getenforce

# Temporary fix
sudo setenforce 0
sudo guestkit inspect vm.qcow2

# Permanent fix (add SELinux policy)
# Or disable SELinux in /etc/selinux/config
```

## Licensing Questions

### What license is guestkit under?

**Apache-2.0** (Apache License, Version 2.0)  
**Copyright owner:** ZyvorAI Labs Private Limited

**What this means:**
- ✅ Free to use commercially
- ✅ Permissive use in proprietary software
- ✅ Can modify for internal use
- ⚠️ Must preserve license notices and NOTICE file in redistributions
- ⚠️ Patent grant applies to contributions

### Can I use guestkit in commercial products?

**Yes!** Apache 2.0 allows commercial use.

**Requirements:**
- Include Apache 2.0 license text and NOTICE (if provided)
- Credit GuestKit and ZyvorAI Labs Private Limited
- Preserve copyright and license notices in redistributions

### Is there commercial support?

**Community support:**
- GitHub Issues: https://github.com/zyvorai/guestkit/issues
- GitHub Discussions: https://github.com/zyvorai/guestkit/discussions

**Commercial support:** Contact ssahani@vmware.com for enterprise support options.

## Feature Requests

### Will guestkit support feature X?

Check the [roadmap](../development/roadmap.md) for planned features.

**Request a feature:**
```bash
# Open feature request on GitHub
gh issue create --title "Feature: My feature" --label enhancement
```

### When will async Python API be available?

**Status:** Code is ready, waiting for pyo3-asyncio to support PyO3 0.22+

**Timeline:** Expected in v0.4.0 (Q2 2026)

**Current workaround:** Use threading in Python:
```python
from concurrent.futures import ThreadPoolExecutor
from guestkit import Guestfs

def inspect_vm(path):
    with Guestfs() as g:
        g.add_drive_ro(path)
        g.launch()
        return g.inspect_os()

with ThreadPoolExecutor(max_workers=4) as executor:
    results = executor.map(inspect_vm, vm_paths)
```

### Will guestkit support ARM/aarch64?

**Partial support** in current version:
- ✅ Can run on ARM hosts
- ✅ Can inspect ARM VM images
- ⚠️ Limited KVM support on ARM

**Full ARM support** planned for v0.5.0.

## Getting Help

### Where can I get help?

1. **Documentation:** Start with [Getting Started Guide](getting-started.md)
2. **FAQ:** This document
3. **GitHub Discussions:** https://github.com/zyvorai/guestkit/discussions
4. **GitHub Issues:** https://github.com/zyvorai/guestkit/issues (for bugs)
5. **Email:** ssahani@vmware.com (for private inquiries)

### How do I report a bug?

```bash
# Use GitHub CLI
gh issue create \
  --title "Bug: Description" \
  --label bug \
  --body "Steps to reproduce:
1. Run guestkit inspect vm.qcow2
2. Error occurs: ...

Expected: ...
Actual: ...

Version: $(guestkit version)
OS: $(uname -a)"
```

**Include:**
- guestkit version
- OS and version
- Disk image format
- Complete error message
- Steps to reproduce

### How can I contribute documentation?

Documentation contributions welcome!

```bash
# Clone repo
git clone https://github.com/zyvorai/guestkit
cd guestkit/docs

# Edit documentation
vim docs/user-guides/my-guide.md

# Submit PR
gh pr create --title "Docs: Improve my-guide"
```

## Quick Links

- [Getting Started](getting-started.md) - Quick start guide
- [CLI Guide](cli-guide.md) - Command reference
- [VM Migration Guide](vm-migration.md) - Migration workflows
- [Troubleshooting](troubleshooting.md) - Problem resolution
- [GitHub Repository](https://github.com/zyvorai/guestkit)
- [Issue Tracker](https://github.com/zyvorai/guestkit/issues)

## Still Have Questions?

If your question isn't answered here:
1. Search [GitHub Discussions](https://github.com/zyvorai/guestkit/discussions)
2. Search [GitHub Issues](https://github.com/zyvorai/guestkit/issues)
3. Ask in [new Discussion](https://github.com/zyvorai/guestkit/discussions/new)
4. For bugs, [create Issue](https://github.com/zyvorai/guestkit/issues/new)

We're here to help! 🚀
