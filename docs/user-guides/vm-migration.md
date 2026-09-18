# VM Migration Guide

Complete guide for migrating VMs across platforms using guestkit.

## Overview

guestkit v0.3.1+ provides powerful VM migration capabilities through:
- **Universal fstab/crypttab Rewriter** - Modify disk mount configurations
- **Device Path Translation** - Automatic device mapping (e.g., /dev/sda → /dev/vda)
- **Windows Registry Modification** - Update Windows system configurations
- **Network Configuration** - Adapt network settings for new environment
- **Boot Configuration** - Modify bootloader for new hypervisor

## Supported Migration Paths

| From | To | Status | Notes |
|------|-----|--------|-------|
| Hyper-V | KVM | ✅ Full | Primary use case with [hyper2kvm](https://github.com/zyvorai/h2kvm) |
| VMware | KVM | ✅ Full | VMDK to QCOW2 conversion supported |
| VirtualBox | KVM | ✅ Full | VDI to QCOW2 conversion supported |
| Physical | KVM (P2V) | ✅ Full | Raw disk imaging supported |
| AWS | Azure | ⚠️ Partial | Network reconfiguration required |
| KVM | KVM | ✅ Full | Cross-host migration |

## Quick Start

### Assurance-first workflow (recommended)

Score boot probability and migration readiness **before** editing the disk:

```bash
# Boot gate + blockers
guestkit doctor vm.vmdk --target proxmox --explain

# Hypervisor-aware checklist (VirtIO, cloud-init, downtime estimate)
guestkit migrate-plan vm.vmdk --target proxmox -o json > migration-plan.json

# Windows-specific signals
guestkit inspect vm.vmdk --profile windows-migration -o json

# Fix boot blockers offline, then re-score
guestkit repair vm.vmdk --fix boot --dry-run
guestkit repair vm.vmdk --fix boot
guestkit doctor vm.vmdk --target proxmox
```

See [Migration assurance](../features/migration-assurance.md) for `fleet analyze`, `policy check`, and `forensic-diff`.

### Live guest agent (optional)

After migration, run assurance **inside** a booted VM via the GuestKit agent (virtio-serial channel `com.zyvor.guestkit.0`):

```bash
# Pre-bake agent during offline prep
guestkit repair vm.qcow2 --fix boot --inject-agent \
  --agent-binary ./target/x86_64-unknown-linux-musl/release/guestkit

# Or include agent ops in exported migrate plan
guestkit migrate-plan vm.qcow2 --target kvm --export plan.yaml --inject-agent

# Host-side bridge (libvirt channel → HTTP)
guestkit agent-proxy \
  --socket /var/lib/libvirt/qemu/channel/target/$VM/com.zyvor.guestkit.0 \
  --listen 127.0.0.1:8765
curl -s http://127.0.0.1:8765/doctor | jq .
```

See [Guest agent](../features/guest-agent.md) for KubeVirt channel YAML, RPC methods, and worker job integration.

**TUI:** `guestctl tui vm.qcow2` → Security → **Assurance** (or Dashboard **`a`**, palette `: doctor`). Preview operations with **`p`** before exporting `migrate-plan --export`.

### Basic Migration Workflow

```bash
# 1. Convert disk format (if needed)
guestkit convert vm.vmdk --output vm.qcow2 --format qcow2

# 2. Inspect source VM
guestkit inspect vm.qcow2 --output json > source-vm.json

# 3. Perform migration modifications (see detailed sections below)

# 4. Verify migrated VM
guestkit inspect vm.qcow2
```

## Detailed Migration Scenarios

### 1. Hyper-V to KVM Migration

Complete workflow for migrating Windows or Linux VMs from Hyper-V to KVM.

#### Prerequisites

```bash
# Install required tools
sudo dnf install qemu-img guestkit

# Export Hyper-V VM (Windows PowerShell on Hyper-V host)
Export-VM -Name "MyVM" -Path "C:\Exports"
```

#### Step 1: Convert VHDX to QCOW2

```bash
# Convert Hyper-V VHDX to KVM QCOW2
qemu-img convert -f vhdx -O qcow2 \
  /path/to/vm.vhdx \
  /path/to/vm.qcow2 \
  -p

# Verify conversion
guestkit detect vm.qcow2
```

#### Step 2: Inspect Source Configuration

```bash
# Get complete VM inventory
guestkit inspect vm.qcow2 --profile migration --output json > vm-inventory.json

# Key information to note:
# - OS type and version
# - Network interfaces (synthetic → virtio)
# - Disk controllers (IDE/SCSI → virtio)
# - Boot configuration
```

#### Step 3: Modify Device Paths (Linux VMs)

For Linux VMs, update device references:

```rust
use guestkit::guestfs::Guestfs;
use std::collections::HashMap;

fn migrate_hyperv_to_kvm() -> Result<(), Box<dyn std::error::Error>> {
    let mut g = Guestfs::new()?;
    g.add_drive("vm.qcow2")?;
    g.launch()?;

    let roots = g.inspect_os()?;
    for root in roots {
        // Device mapping: Hyper-V → KVM
        let mut device_map = HashMap::new();
        device_map.insert("/dev/sda", "/dev/vda");
        device_map.insert("/dev/sdb", "/dev/vdb");

        // Rewrite fstab
        g.rewrite_fstab(&root, &device_map)?;

        // Rewrite crypttab (if encrypted)
        if g.has_luks(&root)? {
            g.rewrite_crypttab(&root, &device_map)?;
        }

        // Update bootloader configuration
        g.update_grub_device_map(&root, &device_map)?;
    }

    g.shutdown()?;
    Ok(())
}
```

#### Step 4: Install VirtIO Drivers (Windows VMs)

Windows VMs need VirtIO drivers for KVM:

```bash
# Download VirtIO driver ISO
wget https://fedorapeople.org/groups/virt/virtio-win/direct-downloads/latest-virtio/virtio-win.iso

# Inject drivers into Windows image
virt-win-reg vm.qcow2 --merge virtio-drivers.reg
```

**VirtIO Driver Registry Import:**

```reg
Windows Registry Editor Version 5.00

[HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Services\vioscsi]
"Start"=dword:00000000
"Type"=dword:00000001

[HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Services\viostor]
"Start"=dword:00000000
"Type"=dword:00000001
```

#### Step 5: Network Configuration

```bash
# For Linux VMs - update network interface names
guestkit interactive vm.qcow2
> mount /
> cat /etc/network/interfaces  # or /etc/sysconfig/network-scripts/
> # Note interface names (eth0 → ens3)
> exit

# Use guestkit to update network configs
# (Automated network migration coming in v0.4.0)
```

#### Step 6: Test Migration

```bash
# Create KVM VM definition
virt-install \
  --name migrated-vm \
  --memory 4096 \
  --vcpus 2 \
  --disk path=vm.qcow2,format=qcow2 \
  --os-variant ubuntu22.04 \
  --network bridge=virbr0 \
  --graphics vnc \
  --boot hd

# Monitor first-boot readiness *before* power-on
guestkit doctor vm.qcow2 --target kvm --explain

# After the guest is running, use the GuestKit agent (not virsh console)
guestkit agent-call --socket /var/lib/libvirt/qemu/channel/target/migrated-vm/org.qemu.guest_agent.0 \
  --method guestkit.getBootAnalysis
```

### 2. VMware to KVM Migration

#### Step 1: Export from VMware

```bash
# Option A: Convert VMDK directly
qemu-img convert -f vmdk -O qcow2 vm.vmdk vm.qcow2

# Option B: Use guestkit
guestkit convert vm.vmdk --output vm.qcow2 --format qcow2 --compress
```

#### Step 2: Device Path Migration

VMware uses different device naming:

```python
# Python migration script
from guestkit import Guestfs

g = Guestfs()
g.add_drive("vm.qcow2")
g.launch()

roots = g.inspect_os()
for root in roots:
    # VMware → KVM device mapping
    device_map = {
        "/dev/sda": "/dev/vda",
        "/dev/sdb": "/dev/vdb",
        # VMware SCSI devices
        "/dev/sd[a-z]": "/dev/vd[a-z]"
    }

    # Update fstab
    g.rewrite_fstab(root, device_map)

    # Update GRUB configuration
    g.update_grub_device_map(root, device_map)

g.shutdown()
```

#### Step 3: Remove VMware Tools (Linux)

```bash
# Interactive mode
guestkit interactive vm.qcow2
> mount /
> command "dpkg --purge open-vm-tools"  # Debian/Ubuntu
> command "rpm -e open-vm-tools"        # RHEL/Fedora
> exit
```

### 3. Physical to Virtual (P2V)

Convert physical servers to KVM virtual machines.

#### Step 1: Create Disk Image from Physical Server

```bash
# On physical server (boot from live USB/CD)
# Compress and stream over network
dd if=/dev/sda bs=4M status=progress | \
  gzip -c | \
  ssh user@kvm-host "gunzip -c > /var/lib/libvirt/images/p2v-server.raw"

# Or use partclone for efficiency (only used blocks)
partclone.ext4 -c -s /dev/sda1 | \
  ssh user@kvm-host "cat > /var/lib/libvirt/images/p2v-server.img"
```

#### Step 2: Convert to QCOW2

```bash
# On KVM host
qemu-img convert -f raw -O qcow2 \
  p2v-server.raw \
  p2v-server.qcow2 \
  -p -c  # Progress and compression
```

#### Step 3: Hardware Adaptation

Physical servers need hardware driver changes:

```rust
use guestkit::guestfs::Guestfs;

fn adapt_p2v_hardware() -> Result<(), Box<dyn std::error::Error>> {
    let mut g = Guestfs::new()?;
    g.add_drive("p2v-server.qcow2")?;
    g.launch()?;

    let roots = g.inspect_os()?;
    for root in &roots {
        // Remove physical hardware drivers
        g.command(&[
            "modprobe", "-r",
            "e1000e",     // Intel NIC
            "megaraid",   // RAID controller
            "hpsa"        // HP Smart Array
        ])?;

        // Add VirtIO drivers to initramfs
        g.command(&["dracut", "-f", "--add-drivers", "virtio_blk virtio_net"])?;

        // Update network configuration
        // Physical: eth0 (e1000e) → Virtual: ens3 (virtio)
        g.update_network_config(root, "eth0", "ens3")?;
    }

    g.shutdown()?;
    Ok(())
}
```

### 4. Cloud Migration (AWS → Azure)

Cross-cloud VM migration.

#### Challenges

- **Network Configuration**: Different metadata services
- **Disk Naming**: Different device naming conventions
- **Boot Configuration**: Different bootloaders
- **Agents**: Cloud-specific agents (AWS SSM → Azure VM Agent)

#### Migration Steps

```bash
# 1. Export from AWS
aws ec2 create-instance-export-task \
  --instance-id i-1234567890abcdef0 \
  --target-environment vmware \
  --export-to-s3-task file://export-task.json

# 2. Download and convert
aws s3 cp s3://my-bucket/vm.vmdk .
qemu-img convert -f vmdk -O qcow2 vm.vmdk vm.qcow2

# 3. Remove AWS-specific configuration
guestkit interactive vm.qcow2
> mount /
> command "systemctl disable amazon-ssm-agent"
> command "rm -f /etc/cloud/cloud.cfg.d/91-aws.cfg"
> exit

# 4. Add Azure configuration
# (Azure VM Agent installation)

# 5. Upload to Azure
# Convert to VHD format for Azure
qemu-img convert -f qcow2 -O vpc vm.qcow2 vm.vhd
az disk create --resource-group myRG --name myDisk --source vm.vhd
```

## Advanced Migration Techniques

### Encrypted Volume Migration

Migrating LUKS encrypted volumes:

```rust
use guestkit::guestfs::Guestfs;

fn migrate_luks_volumes() -> Result<(), Box<dyn std::error::Error>> {
    let mut g = Guestfs::new()?;
    g.add_drive("encrypted-vm.qcow2")?;
    g.launch()?;

    // Open LUKS volume with passphrase
    g.luks_open("/dev/sda2", "luks-root", "passphrase")?;

    let roots = g.inspect_os()?;
    for root in roots {
        // Update crypttab with new device names
        let device_map = vec![
            ("/dev/sda2", "/dev/vda2"),
        ];
        g.rewrite_crypttab(&root, &device_map)?;

        // Ensure initramfs includes new device support
        g.command(&["update-initramfs", "-u"])?;
    }

    g.luks_close("luks-root")?;
    g.shutdown()?;
    Ok(())
}
```

### LVM Volume Migration

```bash
# Inspect LVM configuration
guestkit inspect vm.qcow2 --profile migration | jq '.lvm'

# Migration with guestkit
guestkit interactive vm.qcow2
> lvm-scan
> lvs
> # Note volume group and logical volume names
> # They typically don't need changes, but verify UUIDs
> exit
```

### Multi-Disk Migration

VMs with multiple disks:

```rust
fn migrate_multi_disk() -> Result<(), Box<dyn std::error::Error>> {
    let mut g = Guestfs::new()?;

    // Add all disks
    g.add_drive("disk1.qcow2")?;
    g.add_drive("disk2.qcow2")?;
    g.add_drive("disk3.qcow2")?;

    g.launch()?;

    // Device mapping for all disks
    let device_map = vec![
        ("/dev/sda", "/dev/vda"),
        ("/dev/sdb", "/dev/vdb"),
        ("/dev/sdc", "/dev/vdc"),
    ];

    let roots = g.inspect_os()?;
    for root in roots {
        g.rewrite_fstab(&root, &device_map)?;
    }

    g.shutdown()?;
    Ok(())
}
```

## Migration Checklist

### Pre-Migration

- [ ] Backup original VM
- [ ] Document current configuration (network, storage, services)
- [ ] Check disk space on target
- [ ] Verify guestkit version (0.3.1+)
- [ ] Test migration on non-production VM first

### During Migration

- [ ] Convert disk format
- [ ] Inspect source VM configuration
- [ ] Map device paths (source → target)
- [ ] Update fstab
- [ ] Update crypttab (if encrypted)
- [ ] Update bootloader configuration
- [ ] Install/update drivers (VirtIO for Windows)
- [ ] Remove source hypervisor tools
- [ ] Update network configuration
- [ ] Clean up cloud-specific agents (if applicable)

### Post-Migration

- [ ] Test boot process
- [ ] Verify network connectivity
- [ ] Check disk mounts
- [ ] Verify services start correctly
- [ ] Install target hypervisor tools
- [ ] Update documentation
- [ ] Create new backups

## Troubleshooting

### Boot Failure: "No bootable device"

**Cause:** Boot configuration not updated for new device names

**Solution:**
```bash
guestkit interactive vm.qcow2
> mount /boot
> cat /boot/grub/grub.cfg  # Check device references
> # Update grub.cfg manually or regenerate
> command "grub2-mkconfig -o /boot/grub/grub.cfg"
> exit
```

### Network Not Working After Migration

**Cause:** Interface names changed (eth0 → ens3)

**Solution:**
```bash
# Update network configuration
guestkit interactive vm.qcow2
> mount /
> ls /etc/sysconfig/network-scripts/  # RHEL/Fedora
> ls /etc/network/interfaces.d/        # Debian/Ubuntu
> # Rename interface files: ifcfg-eth0 → ifcfg-ens3
> exit
```

### Windows Blue Screen After Migration

**Cause:** Missing VirtIO drivers

**Solution:**
- Boot Windows in Safe Mode
- Install VirtIO drivers from ISO
- Or: Pre-inject drivers before migration using virt-win-reg

### Encrypted Volume Won't Unlock

**Cause:** Device paths in crypttab don't match new devices

**Solution:**
```bash
# Use UUIDs instead of device paths in crypttab
guestkit interactive vm.qcow2
> mount /
> cat /etc/crypttab
> # Update to use UUID= instead of /dev/sdXN
> blkid  # Get UUIDs
> edit /etc/crypttab
> exit
```

## Performance Optimization

### Disk I/O

```bash
# Use VirtIO for best performance
# In VM XML definition:
<disk type='file' device='disk'>
  <driver name='qemu' type='qcow2' cache='none' io='native'/>
  <source file='/var/lib/libvirt/images/vm.qcow2'/>
  <target dev='vda' bus='virtio'/>
</disk>
```

### Network Performance

```bash
# Use VirtIO network driver
<interface type='bridge'>
  <model type='virtio'/>
  <driver name='vhost' queues='4'/>
</interface>
```

### Post-Migration Optimization

```bash
# Defragment after migration (Windows)
# Run in VM after first boot

# Trim/discard (Linux)
guestkit interactive vm.qcow2
> mount /
> command "fstrim -av"
> exit

# Optimize QCOW2 image
qemu-img convert -O qcow2 -c vm.qcow2 vm-optimized.qcow2
```

## Migration Scripts

### Automated Hyper-V to KVM Migration Script

```bash
#!/bin/bash
# migrate-hyperv-to-kvm.sh

set -e

SOURCE_VHDX="$1"
OUTPUT_QCOW2="$2"

if [ -z "$SOURCE_VHDX" ] || [ -z "$OUTPUT_QCOW2" ]; then
    echo "Usage: $0 <source.vhdx> <output.qcow2>"
    exit 1
fi

echo "Starting migration: $SOURCE_VHDX → $OUTPUT_QCOW2"

# Step 1: Convert format
echo "[1/5] Converting VHDX to QCOW2..."
qemu-img convert -f vhdx -O qcow2 -p "$SOURCE_VHDX" "$OUTPUT_QCOW2"

# Step 2: Inspect VM
echo "[2/5] Inspecting VM..."
guestkit inspect "$OUTPUT_QCOW2" --output json > migration-report.json

# Step 3: Detect OS type
OS_TYPE=$(jq -r '.operating_systems[0].os_type' migration-report.json)
echo "Detected OS: $OS_TYPE"

# Step 4: Modify configuration
if [ "$OS_TYPE" = "linux" ]; then
    echo "[3/5] Updating Linux configuration..."
    guestkit interactive "$OUTPUT_QCOW2" <<EOF
mount /
command "sed -i 's/\/dev\/sda/\/dev\/vda/g' /etc/fstab"
command "update-grub"
exit
EOF
elif [ "$OS_TYPE" = "windows" ]; then
    echo "[3/5] Windows VM detected - manual driver installation required"
    echo "Please install VirtIO drivers after first boot"
fi

# Step 5: Optimize
echo "[4/5] Optimizing QCOW2 image..."
qemu-img convert -O qcow2 -c -p "$OUTPUT_QCOW2" "${OUTPUT_QCOW2}.optimized"
mv "${OUTPUT_QCOW2}.optimized" "$OUTPUT_QCOW2"

echo "[5/5] Migration complete!"
echo "Next steps:"
echo "  1. Create KVM VM definition"
echo "  2. Test boot process"
echo "  3. Verify network and services"
```

## Best Practices

1. **Always backup before migration** - Keep original VM until migration is verified
2. **Test on non-production first** - Validate migration process with test VM
3. **Document source configuration** - Capture complete inventory with `--profile migration`
4. **Use UUIDs for mounts** - More reliable than device paths across platforms
5. **Verify after each step** - Use `guestkit inspect` to check changes
6. **Keep migration logs** - Save all output for troubleshooting
7. **Plan for rollback** - Have recovery plan if migration fails

## API Reference

### Migration Functions

```rust
// Rewrite fstab with device mapping
pub fn rewrite_fstab(&mut self, root: &str, device_map: &HashMap<&str, &str>) -> Result<()>

// Rewrite crypttab for encrypted volumes
pub fn rewrite_crypttab(&mut self, root: &str, device_map: &HashMap<&str, &str>) -> Result<()>

// Update GRUB device mappings
pub fn update_grub_device_map(&mut self, root: &str, device_map: &HashMap<&str, &str>) -> Result<()>

// Update network interface configuration
pub fn update_network_config(&mut self, root: &str, old_iface: &str, new_iface: &str) -> Result<()>

// Check for LUKS encryption
pub fn has_luks(&mut self, root: &str) -> Result<bool>
```

## Further Reading

- [Dump virsh → GuestKit](virsh-to-guestkit.md) — replace `virsh qemu-agent-command`
- [QEMU / VirtIO runtime](../features/qemu-runtime.md) — assurance-gated local launch
- [h2kvm integration](../features/hyper2kvm-integration.md)
- [hyper2kvm Project](https://github.com/zyvorai/h2kvm) - Production Hyper-V to KVM migration
- [VirtIO Driver Installation](https://docs.fedoraproject.org/en-US/quick-docs/creating-windows-virtual-machines-using-virtio-drivers/)
- [KVM Networking Guide](https://wiki.libvirt.org/page/Networking)
- [QEMU Image Formats](https://qemu.readthedocs.io/en/latest/system/images.html)

## Support

For migration issues:
- GitHub Issues: https://github.com/zyvorai/guestkit/issues
- Tag with: `migration`, `hyper-v`, `vmware`, etc.
- Include: OS type, source platform, target platform, error messages
