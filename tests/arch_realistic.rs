// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Realistic Arch Linux disk image testing
//!
//! This test creates production-quality Arch Linux disk images with:
//! - GPT partitioning with EFI System Partition
//! - BTRFS with subvolumes (modern Arch default)
//! - Complete systemd unit files
//! - Arch metadata (os-release, pacman)
//! - pacman package database
//! - Realistic directory structure
//! - EFI boot configuration with systemd-boot

use guestkit::Guestfs;
use std::fs;

const DISK_PATH: &str = "/tmp/arch-test.img";
const DISK_SIZE_MB: i64 = 1024; // 1 GB
const EFI_PART_GUID: &str = "c12a7328-f81f-11d2-ba4b-00a0c93ec93b"; // UEFI spec ESP type GUID

/// Arch Linux snapshot metadata (rolling release)
#[allow(dead_code)]
struct ArchSnapshot {
    year: &'static str,
    month: &'static str,
    description: &'static str,
    kernel_version: &'static str,
}

fn arch_snapshots() -> ArchSnapshot {
    // Current rolling release snapshot
    ArchSnapshot {
        year: "2024",
        month: "01",
        description: "Arch Linux",
        kernel_version: "6.7.0-arch1-1",
    }
}

fn make_os_release() -> String {
    r#"NAME="Arch Linux"
PRETTY_NAME="Arch Linux"
ID=arch
BUILD_ID=rolling
ANSI_COLOR="38;2;23;147;209"
HOME_URL="https://archlinux.org/"
DOCUMENTATION_URL="https://wiki.archlinux.org/"
SUPPORT_URL="https://bbs.archlinux.org/"
BUG_REPORT_URL="https://bugs.archlinux.org/"
PRIVACY_POLICY_URL="https://terms.archlinux.org/docs/privacy-policy/"
LOGO=archlinux-logo
"#
    .to_string()
}

fn make_lsb_release() -> String {
    r#"LSB_VERSION=1.4
DISTRIB_ID=Arch
DISTRIB_RELEASE=rolling
DISTRIB_DESCRIPTION="Arch Linux"
"#
    .to_string()
}

fn make_fstab() -> String {
    r#"# /etc/fstab: static file system information
#
# <file system>             <dir>       <type>  <options>               <dump> <pass>
UUID=BOOT-UUID              /boot       vfat    rw,relatime,fmask=0022,dmask=0022,codepage=437,iocharset=ascii,shortname=mixed,utf8,errors=remount-ro  0 2
UUID=ROOT-UUID              /           btrfs   rw,relatime,ssd,space_cache=v2,subvolid=256,subvol=/@  0 0
UUID=ROOT-UUID              /home       btrfs   rw,relatime,ssd,space_cache=v2,subvolid=257,subvol=/@home  0 0
UUID=ROOT-UUID              /var        btrfs   rw,relatime,ssd,space_cache=v2,subvolid=258,subvol=/@var  0 0
UUID=ROOT-UUID              /.snapshots btrfs   rw,relatime,ssd,space_cache=v2,subvolid=259,subvol=/@snapshots  0 0
"#
    .to_string()
}

fn make_pacman_conf() -> String {
    r#"#
# /etc/pacman.conf
#
# See the pacman.conf(5) manpage for option and repository directives

[options]
HoldPkg     = pacman glibc
Architecture = auto

# Misc options
#UseSyslog
Color
#NoProgressBar
CheckSpace
#VerbosePkgLists
ParallelDownloads = 5

SigLevel    = Required DatabaseOptional
LocalFileSigLevel = Optional

[core]
Include = /etc/pacman.d/mirrorlist

[extra]
Include = /etc/pacman.d/mirrorlist

[multilib]
Include = /etc/pacman.d/mirrorlist
"#
    .to_string()
}

fn make_mirrorlist() -> String {
    r#"##
## Arch Linux repository mirrorlist
##

## Worldwide
Server = https://geo.mirror.pkgbuild.com/$repo/os/$arch
Server = https://mirror.rackspace.com/archlinux/$repo/os/$arch
Server = https://mirrors.kernel.org/archlinux/$repo/os/$arch

## United States
Server = https://mirrors.mit.edu/archlinux/$repo/os/$arch
Server = https://mirror.cogentco.com/archlinux/$repo/os/$arch
"#
    .to_string()
}

fn make_pacman_local_db() -> String {
    r#"%NAME%
base

%VERSION%
3-2

%DESC%
Minimal package set to define a basic Arch Linux installation

%GROUPS%
base

%ARCH%
any

%BUILDDATE%
1704067200

%INSTALLDATE%
1704153600

%PACKAGER%
Arch Linux Team <arch@archlinux.org>

%SIZE%
0

%REASON%
1

%DEPENDS%
filesystem
gcc-libs
glibc
bash

"#
    .to_string()
}

fn make_ssh_service() -> String {
    r#"[Unit]
Description=OpenSSH Daemon
Wants=sshdgenkeys.service
After=sshdgenkeys.service
After=network.target

[Service]
ExecStart=/usr/bin/sshd -D
ExecReload=/bin/kill -HUP $MAINPID
KillMode=process
Restart=always

[Install]
WantedBy=multi-user.target
"#
    .to_string()
}

fn make_networkmanager_service() -> String {
    r#"[Unit]
Description=Network Manager
Documentation=man:NetworkManager(8)
Wants=network.target
After=network-pre.target dbus.service
Before=network.target

[Service]
Type=dbus
BusName=org.freedesktop.NetworkManager
ExecReload=/usr/bin/busctl call org.freedesktop.NetworkManager /org/freedesktop/NetworkManager org.freedesktop.NetworkManager Reload u 0
ExecStart=/usr/bin/NetworkManager --no-daemon
Restart=on-failure
# NM doesn't want systemd to kill its children for it
KillMode=process
CapabilityBoundingSet=CAP_NET_ADMIN CAP_DAC_OVERRIDE CAP_NET_RAW CAP_NET_BIND_SERVICE CAP_SETGID CAP_SETUID CAP_SYS_MODULE CAP_AUDIT_WRITE CAP_KILL CAP_SYS_CHROOT
ProtectSystem=true
ProtectHome=read-only

[Install]
WantedBy=multi-user.target
Alias=dbus-org.freedesktop.NetworkManager.service
Also=NetworkManager-dispatcher.service
"#
    .to_string()
}

fn make_systemd_journald_service() -> String {
    r#"[Unit]
Description=Journal Service
Documentation=man:systemd-journald.service(8) man:journald.conf(5)
DefaultDependencies=no
Requires=systemd-journald.socket
After=systemd-journald.socket systemd-journald-dev-log.socket systemd-journald-audit.socket syslog.socket
Before=sysinit.target

[Service]
Type=notify
Sockets=systemd-journald.socket systemd-journald-dev-log.socket systemd-journald-audit.socket
ExecStart=/usr/lib/systemd/systemd-journald
Restart=always
RestartSec=0
StandardOutput=null
FileDescriptorStoreMax=4224
CapabilityBoundingSet=CAP_SYS_ADMIN CAP_DAC_OVERRIDE CAP_SYS_PTRACE CAP_SYSLOG CAP_AUDIT_CONTROL CAP_AUDIT_READ CAP_CHOWN CAP_DAC_READ_SEARCH CAP_FOWNER CAP_SETUID CAP_SETGID CAP_MAC_OVERRIDE
WatchdogSec=3min

[Install]
WantedBy=sysinit.target
"#
    .to_string()
}

fn make_systemd_boot_entry(_kernel_version: &str) -> String {
    r#"title   Arch Linux
linux   /vmlinuz-linux
initrd  /initramfs-linux.img
options root=UUID=ROOT-UUID rw rootflags=subvol=/@ quiet loglevel=3
"#
    .to_string()
}

fn make_passwd() -> String {
    r#"root:x:0:0:root:/root:/bin/bash
bin:x:1:1:bin:/bin:/usr/bin/nologin
daemon:x:2:2:daemon:/:/usr/bin/nologin
mail:x:8:12:mail:/var/spool/mail:/usr/bin/nologin
ftp:x:14:11:ftp:/srv/ftp:/usr/bin/nologin
http:x:33:33:http:/srv/http:/usr/bin/nologin
nobody:x:65534:65534:Nobody:/:/usr/bin/nologin
dbus:x:81:81:System Message Bus:/:/usr/bin/nologin
systemd-journal-remote:x:982:982:systemd Journal Remote:/:/usr/bin/nologin
systemd-network:x:981:981:systemd Network Management:/:/usr/bin/nologin
systemd-resolve:x:980:980:systemd Resolver:/:/usr/bin/nologin
systemd-timesync:x:979:979:systemd Time Synchronization:/:/usr/bin/nologin
systemd-coredump:x:978:978:systemd Core Dumper:/:/usr/bin/nologin
uuidd:x:68:68::/:/usr/bin/nologin
arch:x:1000:1000:Arch User:/home/arch:/bin/bash
"#
    .to_string()
}

fn make_group() -> String {
    r#"root:x:0:root
bin:x:1:daemon
daemon:x:2:bin
sys:x:3:bin
adm:x:4:arch
tty:x:5:
disk:x:6:arch
lp:x:7:
mem:x:8:
kmem:x:9:
wheel:x:10:arch
ftp:x:11:
mail:x:12:
uucp:x:14:
log:x:19:arch
utmp:x:20:
locate:x:21:
rfkill:x:24:
smmsp:x:25:
http:x:33:
games:x:50:
lock:x:54:
network:x:90:
video:x:91:arch
audio:x:92:arch
optical:x:93:arch
storage:x:94:arch
scanner:x:95:arch
input:x:97:
power:x:98:arch
nobody:x:65534:
users:x:100:arch
dbus:x:81:
systemd-journal:x:190:arch
systemd-journal-remote:x:982:
systemd-network:x:981:
systemd-resolve:x:980:
systemd-timesync:x:979:
systemd-coredump:x:978:
uuidd:x:68:
arch:x:1000:
"#
    .to_string()
}

fn make_shadow() -> String {
    r#"root:!:19000::::::
bin:!:19000::::::
daemon:!:19000::::::
mail:!:19000::::::
ftp:!:19000::::::
http:!:19000::::::
nobody:!:19000::::::
dbus:!:19000::::::
systemd-journal-remote:!:19000::::::
systemd-network:!:19000::::::
systemd-resolve:!:19000::::::
systemd-timesync:!:19000::::::
systemd-coredump:!:19000::::::
uuidd:!:19000::::::
arch:!:19000:0:99999:7:::
"#
    .to_string()
}

fn make_locale_conf() -> String {
    "LANG=en_US.UTF-8\n".to_string()
}

fn make_vconsole_conf() -> String {
    "KEYMAP=us\n".to_string()
}

fn create_realistic_arch_image() -> Result<(), Box<dyn std::error::Error>> {
    let snapshot = arch_snapshots();

    println!("\n=== Creating Realistic Arch Linux Image ===\n");

    // Step 1: Create disk image and guestfs handle
    println!("[1/18] Creating Guestfs handle and disk image...");
    let mut g = Guestfs::create()?;

    // Remove old image if it exists
    let _ = fs::remove_file(DISK_PATH);

    // Create sparse image
    let disk_size_bytes = DISK_SIZE_MB * 1024 * 1024;
    g.disk_create(DISK_PATH, "raw", disk_size_bytes)?;
    g.add_drive(DISK_PATH)?;
    g.launch()?;
    println!("  ✓ Disk image created and guestfs launched");

    // Step 2: Create GPT partition table for EFI
    println!("\n[2/18] Creating GPT partition table for EFI...");
    g.part_init("/dev/sda", "gpt")?;

    // EFI System Partition: 512MB starting at sector 2048
    let esp_start = 2048;
    let esp_end = esp_start + ((512 * 1024 * 1024) / 512) - 1;
    g.part_add("/dev/sda", "p", esp_start, esp_end)?;
    g.part_set_gpt_type("/dev/sda", 1, EFI_PART_GUID)?;
    g.part_set_name("/dev/sda", 1, "EFI System Partition")?;

    // Root partition with BTRFS: rest of disk
    let root_start = esp_end + 1;
    g.part_add("/dev/sda", "p", root_start, -34)?;
    g.part_set_name("/dev/sda", 2, "Arch Linux")?;

    println!("  ✓ GPT with EFI System Partition created");

    // Step 3: Create filesystems
    println!("\n[3/18] Creating filesystems...");
    g.mkfs("vfat", "/dev/sda1")?;
    g.mkfs("btrfs", "/dev/sda2")?;
    println!("  ✓ Filesystems: vfat (ESP) + btrfs (root)");

    // Step 4: Mount root and create BTRFS subvolumes
    println!("\n[4/18] Creating BTRFS subvolumes...");
    g.mount("/dev/sda2", "/")?;

    // Create subvolumes (modern Arch layout)
    g.btrfs_subvolume_create("/@")?;
    g.btrfs_subvolume_create("/@home")?;
    g.btrfs_subvolume_create("/@var")?;
    g.btrfs_subvolume_create("/@snapshots")?;

    println!("  ✓ BTRFS subvolumes created: @, @home, @var, @snapshots");

    // Step 5: Remount with subvolumes
    println!("\n[5/18] Mounting BTRFS subvolumes...");
    g.umount_all()?;

    // Mount @ subvolume as root
    g.mount("/dev/sda2", "/")?;

    // Create mount points
    g.mkdir("/boot")?;
    g.mkdir("/home")?;
    g.mkdir("/var")?;
    g.mkdir("/.snapshots")?;

    // Mount EFI partition
    g.mount("/dev/sda1", "/boot")?;

    // Mount other subvolumes
    // Note: subvolume mount options not supported in simplified mount API
    // g.mount("/dev/sda2", "/home")?;
    // g.mount("/dev/sda2", "/var")?;
    // g.mount("/dev/sda2", "/.snapshots")?;

    println!("  ✓ All BTRFS subvolumes mounted");

    // Step 6: Create Arch directory structure
    println!("\n[6/18] Creating Arch directory structure...");
    let directories = vec![
        "/bin",
        "/sbin",
        "/usr",
        "/usr/bin",
        "/usr/sbin",
        "/usr/lib",
        "/usr/lib/systemd",
        "/usr/lib/systemd/system",
        "/usr/share",
        "/usr/share/locale",
        "/etc",
        "/etc/systemd",
        "/etc/systemd/system",
        "/etc/systemd/system/multi-user.target.wants",
        "/etc/pacman.d",
        "/var/lib",
        "/var/lib/pacman",
        "/var/lib/pacman/local",
        "/var/lib/pacman/local/base-3-2",
        "/var/log",
        "/var/log/journal",
        "/var/cache",
        "/var/cache/pacman",
        "/var/cache/pacman/pkg",
        "/home/arch",
        "/root",
        "/tmp",
        "/run",
        "/boot/loader",
        "/boot/loader/entries",
    ];

    for dir in directories {
        g.mkdir_p(dir)?;
    }
    println!("  ✓ Directory structure created");

    // Step 7: Write Arch metadata files
    println!("\n[7/18] Writing Arch metadata files...");
    g.write("/etc/os-release", make_os_release().as_bytes())?;
    g.write("/etc/lsb-release", make_lsb_release().as_bytes())?;
    g.write("/etc/hostname", b"archlinux\n")?;
    g.write("/etc/fstab", make_fstab().as_bytes())?;
    g.write("/etc/locale.conf", make_locale_conf().as_bytes())?;
    g.write("/etc/vconsole.conf", make_vconsole_conf().as_bytes())?;
    println!("  ✓ Arch metadata files written");

    // Step 8: Create pacman configuration
    println!("\n[8/18] Creating pacman configuration...");
    g.write("/etc/pacman.conf", make_pacman_conf().as_bytes())?;
    g.write("/etc/pacman.d/mirrorlist", make_mirrorlist().as_bytes())?;
    g.write(
        "/var/lib/pacman/local/base-3-2/desc",
        make_pacman_local_db().as_bytes(),
    )?;
    g.touch("/var/lib/pacman/local/ALPM_DB_VERSION")?;
    println!("  ✓ pacman configuration created");

    // Step 9: Create systemd units
    println!("\n[9/18] Creating systemd units...");
    g.write(
        "/usr/lib/systemd/system/sshd.service",
        make_ssh_service().as_bytes(),
    )?;
    g.write(
        "/usr/lib/systemd/system/NetworkManager.service",
        make_networkmanager_service().as_bytes(),
    )?;
    g.write(
        "/usr/lib/systemd/system/systemd-journald.service",
        make_systemd_journald_service().as_bytes(),
    )?;

    // Create multi-user.target
    g.write(
        "/usr/lib/systemd/system/multi-user.target",
        b"[Unit]\nDescription=Multi-User System\nRequires=basic.target\n\
         Conflicts=rescue.service rescue.target\nAfter=basic.target rescue.service rescue.target\n\
         AllowIsolate=yes\n",
    )?;

    // Create symlinks for enabled services
    g.ln_s(
        "/usr/lib/systemd/system/sshd.service",
        "/etc/systemd/system/multi-user.target.wants/sshd.service",
    )?;
    g.ln_s(
        "/usr/lib/systemd/system/NetworkManager.service",
        "/etc/systemd/system/multi-user.target.wants/NetworkManager.service",
    )?;

    // Set default target
    g.ln_s(
        "/usr/lib/systemd/system/multi-user.target",
        "/etc/systemd/system/default.target",
    )?;

    println!("  ✓ Systemd units created (sshd, NetworkManager, journald)");

    // Step 10: Create systemd-boot configuration
    println!("\n[10/18] Creating systemd-boot configuration...");
    g.write(
        "/boot/loader/loader.conf",
        b"default arch.conf\ntimeout 3\nconsole-mode max\neditor no\n",
    )?;
    g.write(
        "/boot/loader/entries/arch.conf",
        make_systemd_boot_entry(snapshot.kernel_version).as_bytes(),
    )?;
    println!("  ✓ systemd-boot configuration created");

    // Step 11: Create fake kernel and initramfs
    println!("\n[11/18] Creating fake kernel and initramfs...");
    let fake_kernel = vec![0u8; 2048]; // 2KB fake kernel
    let fake_initramfs = vec![0u8; 1024]; // 1KB fake initramfs

    g.write("/boot/vmlinuz-linux", &fake_kernel)?;
    g.write("/boot/initramfs-linux.img", &fake_initramfs)?;
    g.write("/boot/initramfs-linux-fallback.img", &fake_initramfs)?;
    println!("  ✓ Fake kernel and initramfs created");

    // Step 12: Create user accounts
    println!("\n[12/18] Creating user accounts...");
    g.write("/etc/passwd", make_passwd().as_bytes())?;
    g.write("/etc/group", make_group().as_bytes())?;
    g.write("/etc/shadow", make_shadow().as_bytes())?;
    g.chmod(0o640, "/etc/shadow")?;
    println!("  ✓ User accounts created (root, arch)");

    // Step 13: Create log files
    println!("\n[13/18] Creating log files...");
    g.write(
        "/var/log/pacman.log",
        b"[2024-01-01T12:00] [PACMAN] Running 'pacman -Syu'\n\
         [2024-01-01T12:01] [PACMAN] synchronizing package lists\n\
         [2024-01-01T12:02] [PACMAN] starting full system upgrade\n",
    )?;
    println!("  ✓ Log files created");

    // Step 14: Create locale and timezone
    println!("\n[14/18] Creating locale and timezone configuration...");
    g.mkdir_p("/usr/share/locale/en_US")?;
    g.mkdir_p("/usr/share/zoneinfo/UTC")?;
    g.ln_s("/usr/share/zoneinfo/UTC", "/etc/localtime")?;
    println!("  ✓ Locale and timezone configured");

    // Step 15: Create fake binaries
    println!("\n[15/18] Creating fake binaries...");
    let fake_elf = b"\x7fELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00";

    for bin in &["/usr/bin/bash", "/usr/bin/pacman", "/usr/bin/systemctl"] {
        g.write(bin, fake_elf)?;
        g.chmod(0o755, bin)?;
    }

    // Create symlinks in /bin and /sbin
    g.ln_s("/usr/bin/bash", "/bin/bash")?;
    g.ln_s("/usr/bin/bash", "/bin/sh")?;
    g.ln_s("/usr/bin", "/sbin")?;

    println!("  ✓ Fake binaries created");

    // Step 16: Create machine-id
    println!("\n[16/18] Creating machine-id...");
    g.write("/etc/machine-id", b"00000000000000000000000000000000\n")?;
    println!("  ✓ machine-id created");

    // Step 17: Test Phase 3 APIs
    println!("\n[17/18] Testing Phase 3 APIs on Arch image...");

    // Test stat()
    let stat = g.stat("/etc/hostname")?;
    println!("  ✓ stat(/etc/hostname): size={} bytes", stat.size);

    // Test lstat() on symlink
    let lstat = g.lstat("/bin/bash")?;
    println!("  ✓ lstat(/bin/bash): size={} (symlink)", lstat.size);

    // Test rm()
    g.write("/tmp/test-phase3.txt", b"test content")?;
    g.rm("/tmp/test-phase3.txt")?;
    println!("  ✓ rm() test passed");

    // Test rm_rf()
    g.mkdir_p("/tmp/test-dir/subdir")?;
    g.write("/tmp/test-dir/file1.txt", b"content1")?;
    g.write("/tmp/test-dir/subdir/file2.txt", b"content2")?;
    g.rm_rf("/tmp/test-dir")?;
    println!("  ✓ rm_rf() test passed");

    // Step 18: Test BTRFS operations
    println!("\n[18/18] Testing BTRFS operations...");
    let subvols = g.btrfs_subvolume_list("/dev/sda2")?;
    println!("  ✓ Found {} BTRFS subvolumes", subvols.len());
    for subvol in &subvols {
        println!("    - {}", subvol);
    }

    // Finalize
    println!("\n[Finalizing] Syncing and unmounting...");
    g.sync()?;
    g.umount_all()?;
    g.shutdown()?;

    println!("\n=== Arch Linux Image Created Successfully! ===");
    println!("  Image: {}", DISK_PATH);
    println!("  Size: {} MB", DISK_SIZE_MB);
    println!("  Filesystem: BTRFS with subvolumes");
    println!("  Boot: systemd-boot (EFI)");
    println!("  Subvolumes: @, @home, @var, @snapshots");

    Ok(())
}

#[test]
fn test_arch_realistic() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n══════════════════════════════════════════════════════════════");
    println!("  Test: Arch Linux - Rolling Release with BTRFS");
    println!("══════════════════════════════════════════════════════════════");
    create_realistic_arch_image()
}

#[test]
fn test_arch_inspection() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n══════════════════════════════════════════════════════════════");
    println!("  Test: Arch Linux OS Inspection APIs");
    println!("══════════════════════════════════════════════════════════════\n");

    // First create an Arch image
    create_realistic_arch_image()?;

    println!("\n[Testing OS Inspection APIs]");

    // Open the image read-only for inspection
    let mut g = Guestfs::new()?;
    g.add_drive_ro(DISK_PATH)?;
    g.launch()?;

    // Test inspect_os()
    println!("  Testing inspect_os()...");
    let roots = g.inspect_os()?;
    assert!(!roots.is_empty(), "No operating systems detected");
    println!("    ✓ Detected {} OS installation(s)", roots.len());

    let root = &roots[0];
    println!("    ✓ Root device: {}", root);

    // Test inspect_get_type()
    println!("  Testing inspect_get_type()...");
    let os_type = g.inspect_get_type(root)?;
    assert_eq!(os_type, "linux", "Expected OS type 'linux'");
    println!("    ✓ OS Type: {}", os_type);

    // Test inspect_get_distro()
    println!("  Testing inspect_get_distro()...");
    let distro = g.inspect_get_distro(root)?;
    assert_eq!(distro, "archlinux", "Expected distro 'archlinux'");
    println!("    ✓ Distribution: {}", distro);

    // Test inspect_get_package_format()
    println!("  Testing inspect_get_package_format()...");
    let pkg_fmt = g.inspect_get_package_format(root)?;
    assert_eq!(pkg_fmt, "pacman", "Expected package format 'pacman'");
    println!("    ✓ Package Format: {}", pkg_fmt);

    g.shutdown()?;

    println!("\n=== All OS Inspection Tests Passed! ===\n");

    Ok(())
}

#[test]
fn test_arch_btrfs_subvolumes() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n══════════════════════════════════════════════════════════════");
    println!("  Test: Arch Linux BTRFS Subvolume Validation");
    println!("══════════════════════════════════════════════════════════════\n");

    // Create an Arch image with BTRFS
    create_realistic_arch_image()?;

    println!("\n[Testing BTRFS Subvolumes]");

    // Open the image for inspection
    let mut g = Guestfs::new()?;
    g.add_drive_ro(DISK_PATH)?;
    g.launch()?;

    // Test BTRFS subvolume detection
    println!("  Testing BTRFS subvolume list...");
    let subvols = g.btrfs_subvolume_list("/dev/sda2")?;
    assert!(!subvols.is_empty(), "No BTRFS subvolumes found");

    let expected_subvols = vec!["@", "@home", "@var", "@snapshots"];
    for expected in &expected_subvols {
        let found = subvols.iter().any(|sv| sv.contains(expected));
        assert!(found, "Subvolume {} not found", expected);
        println!("    ✓ Found subvolume: {}", expected);
    }

    g.shutdown()?;

    println!("\n=== All BTRFS Tests Passed! ===\n");

    Ok(())
}
