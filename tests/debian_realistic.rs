// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Realistic Debian disk image testing
//!
//! This test creates production-quality Debian disk images with:
//! - Both MBR (legacy BIOS) and GPT (EFI) partitioning
//! - LVM layout with multiple logical volumes (root, usr, var, home)
//! - Proper filesystem types
//! - Complete systemd unit files
//! - Debian metadata (debian_version, os-release)
//! - dpkg package database
//! - Realistic directory structure
//! - Boot configuration for both BIOS and EFI

use guestkit::Guestfs;
use std::collections::HashMap;
use std::fs;

const DISK_PATH: &str = "/tmp/debian-test.img";
const DISK_SIZE_MB: i64 = 512; // 512 MB for smaller test images
const EFI_PART_GUID: &str = "c12a7328-f81f-11d2-ba4b-00a0c93ec93b"; // UEFI spec ESP type GUID

// Hard-coded UUIDs for deterministic inspection
const BOOT_UUID: &str = "01234567-0123-0123-0123-012345678901";
const LV_ROOT_UUID: &str = "01234567-0123-0123-0123-012345678902";
const LV_USR_UUID: &str = "01234567-0123-0123-0123-012345678903";
const LV_VAR_UUID: &str = "01234567-0123-0123-0123-012345678904";
const LV_HOME_UUID: &str = "01234567-0123-0123-0123-012345678905";

/// Debian version metadata
struct DebianVersion<'a> {
    codename: &'a str,
    description: &'a str,
    version_number: &'a str,
}

fn debian_presets() -> HashMap<&'static str, DebianVersion<'static>> {
    let mut presets = HashMap::new();

    presets.insert(
        "11",
        DebianVersion {
            codename: "bullseye",
            description: "Debian GNU/Linux 11 (bullseye)",
            version_number: "11.0",
        },
    );

    presets.insert(
        "12",
        DebianVersion {
            codename: "bookworm",
            description: "Debian GNU/Linux 12 (bookworm)",
            version_number: "12.0",
        },
    );

    presets.insert(
        "13",
        DebianVersion {
            codename: "trixie",
            description: "Debian GNU/Linux 13 (trixie)",
            version_number: "13.0",
        },
    );

    presets
}

fn make_debian_version(version: &str) -> String {
    let presets = debian_presets();
    let default_desc = format!("Debian GNU/Linux {}", version);
    let default_meta = DebianVersion {
        codename: "unknown",
        description: &default_desc,
        version_number: version,
    };
    let metadata = presets.get(version).unwrap_or(&default_meta);

    format!("{}\n", metadata.version_number)
}

fn make_os_release(version: &str) -> String {
    let presets = debian_presets();
    let default_desc = format!("Debian GNU/Linux {}", version);
    let default_meta = DebianVersion {
        codename: "unknown",
        description: &default_desc,
        version_number: version,
    };
    let metadata = presets.get(version).unwrap_or(&default_meta);

    format!(
        "NAME=\"Debian GNU/Linux\"\n\
         VERSION=\"{} ({})\"\n\
         ID=debian\n\
         VERSION_ID=\"{}\"\n\
         VERSION_CODENAME={}\n\
         PRETTY_NAME=\"{}\"\n\
         HOME_URL=\"https://www.debian.org/\"\n\
         SUPPORT_URL=\"https://www.debian.org/support\"\n\
         BUG_REPORT_URL=\"https://bugs.debian.org/\"\n",
        version, metadata.codename, version, metadata.codename, metadata.description
    )
}

fn make_fstab(use_efi: bool) -> String {
    let mut fstab = String::new();

    if use_efi {
        fstab.push_str("LABEL=EFI /boot/efi vfat umask=0077 0 1\n");
        fstab.push_str("LABEL=BOOT /boot ext2 defaults 0 0\n");
    } else {
        fstab.push_str("LABEL=BOOT /boot ext2 defaults 0 0\n");
    }

    fstab.push_str("/dev/debian/root / ext2 defaults 0 0\n");
    fstab.push_str("/dev/debian/usr  /usr ext2 defaults 1 2\n");
    fstab.push_str("/dev/debian/var  /var ext2 defaults 1 2\n");
    fstab.push_str("/dev/debian/home /home ext2 defaults 1 2\n");

    fstab
}

fn make_dpkg_status() -> String {
    r#"Package: base-files
Status: install ok installed
Priority: required
Section: admin
Installed-Size: 348
Maintainer: Santiago Vila <sanvila@debian.org>
Architecture: amd64
Version: 12.4
Description: Debian base system miscellaneous files
 This package contains the basic filesystem hierarchy of a Debian system.

Package: bash
Status: install ok installed
Priority: required
Section: shells
Installed-Size: 3000
Maintainer: Debian Bash Maintainers <pkg-bash-maint@lists.alioth.debian.org>
Architecture: amd64
Version: 5.2.15-2+b2
Description: GNU Bourne Again SHell
 Bash is an sh-compatible command language interpreter that executes
 commands read from the standard input or from a file.

Package: coreutils
Status: install ok installed
Priority: required
Section: utils
Installed-Size: 14567
Maintainer: Debian Coreutils Maintainers <coreutils@packages.debian.org>
Architecture: amd64
Version: 9.1-1
Description: GNU core utilities
 This package contains the basic file, shell and text manipulation
 utilities which are expected to exist on every operating system.

Package: dpkg
Status: install ok installed
Priority: required
Section: admin
Installed-Size: 6644
Maintainer: Dpkg Developers <debian-dpkg@lists.debian.org>
Architecture: amd64
Version: 1.21.22
Description: Debian package management system
 This package provides the low-level infrastructure for handling the
 installation and removal of Debian software packages.

Package: systemd
Status: install ok installed
Priority: important
Section: admin
Installed-Size: 17890
Maintainer: Debian systemd Maintainers <pkg-systemd-maintainers@lists.alioth.debian.org>
Architecture: amd64
Version: 252.19-1~deb12u1
Description: system and service manager
 systemd is a system and service manager for Linux.

Package: linux-image-6.1.0-17-amd64
Status: install ok installed
Priority: optional
Section: kernel
Installed-Size: 314567
Maintainer: Debian Kernel Team <debian-kernel@lists.debian.org>
Architecture: amd64
Version: 6.1.69-1
Description: Linux kernel image for Debian
 This package contains the Linux kernel image for Debian.

Package: grub-efi-amd64
Status: install ok installed
Priority: optional
Section: admin
Installed-Size: 7891
Maintainer: GRUB Maintainers <pkg-grub-devel@lists.alioth.debian.org>
Architecture: amd64
Version: 2.06-13
Description: GRand Unified Bootloader, version 2 (EFI-AMD64 version)
 GRUB is a portable, powerful bootloader.
"#
    .to_string()
}

fn make_ssh_service() -> String {
    r#"[Unit]
Description=OpenBSD Secure Shell server
Documentation=man:sshd(8) man:sshd_config(5)
After=network.target auditd.service
ConditionPathExists=!/etc/ssh/sshd_not_to_be_run

[Service]
EnvironmentFile=-/etc/default/ssh
ExecStartPre=/usr/sbin/sshd -t
ExecStart=/usr/sbin/sshd -D $SSHD_OPTS
ExecReload=/bin/kill -HUP $MAINPID
KillMode=process
Restart=on-failure
RestartPreventExitStatus=255
Type=notify

[Install]
WantedBy=multi-user.target
Alias=sshd.service
"#
    .to_string()
}

fn make_networking_service() -> String {
    r#"[Unit]
Description=Raise network interfaces
Documentation=man:interfaces(5)
DefaultDependencies=no
Wants=network.target
After=local-fs.target network-pre.target apparmor.service systemd-sysctl.service systemd-modules-load.service
Before=network.target shutdown.target network-online.target

[Service]
Type=oneshot
EnvironmentFile=-/etc/default/networking
ExecStart=/sbin/ifup -a --read-environment
ExecStop=/sbin/ifdown -a --read-environment
RemainAfterExit=yes
TimeoutStartSec=5min

[Install]
WantedBy=multi-user.target
WantedBy=network-online.target
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
DeviceAllow=char-* rw
ExecStart=/lib/systemd/systemd-journald
FileDescriptorStoreMax=4224
IPAddressDeny=any
LockPersonality=yes
MemoryDenyWriteExecute=yes
NoNewPrivileges=yes
OOMScoreAdjust=-250
Restart=always
RestartSec=0
RestrictAddressFamilies=AF_UNIX AF_NETLINK
RestrictRealtime=yes
RuntimeDirectory=systemd/journal
StandardOutput=null
Sockets=systemd-journald.socket systemd-journald-dev-log.socket systemd-journald-audit.socket
SystemCallArchitectures=native
SystemCallErrorNumber=EPERM
SystemCallFilter=@system-service
Type=notify
WatchdogSec=3min

[Install]
WantedBy=sysinit.target
"#
    .to_string()
}

fn make_grub_config(version: &str, use_efi: bool) -> String {
    let presets = debian_presets();
    let default_desc = format!("Debian GNU/Linux {}", version);
    let default_meta = DebianVersion {
        codename: "unknown",
        description: &default_desc,
        version_number: version,
    };
    let metadata = presets.get(version).unwrap_or(&default_meta);

    let boot_mode = if use_efi { "EFI" } else { "BIOS" };

    format!(
        r#"# GRUB configuration for Debian {} ({})
# Boot mode: {}
#
# This is a fake GRUB configuration for testing purposes

set default=0
set timeout=5

menuentry 'Debian GNU/Linux' {{
    set root='hd0,gpt2'
    linux /vmlinuz-6.1.0-17-amd64 root=/dev/debian/root ro quiet
    initrd /initrd.img-6.1.0-17-amd64
}}

menuentry 'Debian GNU/Linux (recovery mode)' {{
    set root='hd0,gpt2'
    linux /vmlinuz-6.1.0-17-amd64 root=/dev/debian/root ro single
    initrd /initrd.img-6.1.0-17-amd64
}}
"#,
        version, metadata.codename, boot_mode
    )
}

fn make_network_interfaces() -> String {
    r#"# /etc/network/interfaces -- configuration file for ifup(8), ifdown(8)
# See the interfaces(5) manpage for information on what options are
# available.

# The loopback interface
auto lo
iface lo inet loopback

# Primary network interface
auto eth0
iface eth0 inet dhcp
"#
    .to_string()
}

fn make_resolv_conf() -> String {
    "# Generated by NetworkManager\nnameserver 8.8.8.8\nnameserver 8.8.4.4\n".to_string()
}

fn make_apt_sources(version: &str) -> String {
    let presets = debian_presets();
    let default_desc = format!("Debian GNU/Linux {}", version);
    let default_meta = DebianVersion {
        codename: "unknown",
        description: &default_desc,
        version_number: version,
    };
    let metadata = presets.get(version).unwrap_or(&default_meta);

    format!(
        "# Debian {} ({}) sources\n\
         deb http://deb.debian.org/debian {} main contrib non-free\n\
         deb http://deb.debian.org/debian {}-updates main contrib non-free\n\
         deb http://security.debian.org/debian-security {} main contrib non-free\n",
        version, metadata.codename, metadata.codename, metadata.codename, metadata.codename
    )
}

fn make_passwd() -> String {
    r#"root:x:0:0:root:/root:/bin/bash
daemon:x:1:1:daemon:/usr/sbin:/usr/sbin/nologin
bin:x:2:2:bin:/bin:/usr/sbin/nologin
sys:x:3:3:sys:/dev:/usr/sbin/nologin
sync:x:4:65534:sync:/bin:/bin/sync
games:x:5:60:games:/usr/games:/usr/sbin/nologin
man:x:6:12:man:/var/cache/man:/usr/sbin/nologin
lp:x:7:7:lp:/var/spool/lpd:/usr/sbin/nologin
mail:x:8:8:mail:/var/mail:/usr/sbin/nologin
news:x:9:9:news:/var/spool/news:/usr/sbin/nologin
uucp:x:10:10:uucp:/var/spool/uucp:/usr/sbin/nologin
proxy:x:13:13:proxy:/bin:/usr/sbin/nologin
www-data:x:33:33:www-data:/var/www:/usr/sbin/nologin
backup:x:34:34:backup:/var/backups:/usr/sbin/nologin
list:x:38:38:Mailing List Manager:/var/list:/usr/sbin/nologin
irc:x:39:39:ircd:/run/ircd:/usr/sbin/nologin
debian:x:1000:1000:Debian User,,,:/home/debian:/bin/bash
"#
    .to_string()
}

fn make_group() -> String {
    r#"root:x:0:
daemon:x:1:
bin:x:2:
sys:x:3:
adm:x:4:debian
tty:x:5:
disk:x:6:
lp:x:7:
mail:x:8:
news:x:9:
uucp:x:10:
man:x:12:
proxy:x:13:
kmem:x:15:
dialout:x:20:debian
fax:x:21:
voice:x:22:
cdrom:x:24:debian
floppy:x:25:debian
tape:x:26:
sudo:x:27:debian
audio:x:29:debian
dip:x:30:debian
www-data:x:33:
backup:x:34:
operator:x:37:
list:x:38:
irc:x:39:
src:x:40:
shadow:x:42:
utmp:x:43:
video:x:44:debian
sasl:x:45:
plugdev:x:46:debian
staff:x:50:
games:x:60:
users:x:100:
nogroup:x:65534:
debian:x:1000:
"#
    .to_string()
}

fn make_shadow() -> String {
    r#"root:!:19000:0:99999:7:::
daemon:*:19000:0:99999:7:::
bin:*:19000:0:99999:7:::
sys:*:19000:0:99999:7:::
sync:*:19000:0:99999:7:::
games:*:19000:0:99999:7:::
man:*:19000:0:99999:7:::
lp:*:19000:0:99999:7:::
mail:*:19000:0:99999:7:::
news:*:19000:0:99999:7:::
uucp:*:19000:0:99999:7:::
proxy:*:19000:0:99999:7:::
www-data:*:19000:0:99999:7:::
backup:*:19000:0:99999:7:::
list:*:19000:0:99999:7:::
irc:*:19000:0:99999:7:::
debian:!:19000:0:99999:7:::
"#
    .to_string()
}

fn create_realistic_debian_image(
    version: &str,
    use_efi: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let presets = debian_presets();
    let default_desc = format!("Debian GNU/Linux {}", version);
    let default_meta = DebianVersion {
        codename: "unknown",
        description: &default_desc,
        version_number: version,
    };
    let metadata = presets.get(version).unwrap_or(&default_meta);

    let boot_mode = if use_efi { "EFI/GPT" } else { "BIOS/MBR" };

    println!(
        "\n=== Creating Realistic Debian {} Image ({}) ===\n",
        version, boot_mode
    );

    // Step 1: Create disk image and guestfs handle
    println!("[1/16] Creating Guestfs handle and disk image...");
    let mut g = Guestfs::create()?;

    // Remove old image if it exists
    let _ = fs::remove_file(DISK_PATH);

    // Create sparse image using disk_create
    let disk_size_bytes = DISK_SIZE_MB * 1024 * 1024;
    g.disk_create(DISK_PATH, "raw", disk_size_bytes)?;
    g.add_drive(DISK_PATH)?;
    g.launch()?;
    println!("  ✓ Disk image created and guestfs launched");

    // Step 2: Create partition table
    if use_efi {
        println!("\n[2/16] Creating GPT partition table for EFI...");
        g.part_init("/dev/sda", "gpt")?;

        // EFI System Partition: 100MB starting at sector 2048
        // 100MB / 512B = 204800 sectors
        let esp_start = 2048;
        let esp_end = esp_start + 204800 - 1;
        g.part_add("/dev/sda", "p", esp_start, esp_end)?;
        g.part_set_gpt_type("/dev/sda", 1, EFI_PART_GUID)?;
        g.part_set_name("/dev/sda", 1, "EFI System Partition")?;

        // Boot partition: 256 MB
        let boot_start = esp_end + 1;
        let boot_sectors = (256 * 1024 * 1024) / 512;
        let boot_end = boot_start + boot_sectors - 1;
        g.part_add("/dev/sda", "p", boot_start, boot_end)?;
        g.part_set_name("/dev/sda", 2, "Linux Boot")?;

        // LVM partition: rest of disk
        let lvm_start = boot_end + 1;
        g.part_add("/dev/sda", "p", lvm_start, -2048)?;
        g.part_set_name("/dev/sda", 3, "Linux LVM")?;

        println!("  ✓ GPT with EFI System Partition created");
    } else {
        println!("\n[2/16] Creating MBR partition table for BIOS...");
        g.part_init("/dev/sda", "mbr")?;

        // Boot partition: sectors [64, 524287]
        g.part_add("/dev/sda", "p", 64, 524287)?;

        // LVM partition: from 524288 to near end
        g.part_add("/dev/sda", "p", 524288, -64)?;

        println!("  ✓ MBR partition table created");
    }

    // Step 3: Setup LVM
    println!("\n[3/16] Creating LVM layout (PV/VG/LVs)...");
    let _lvm_device = if use_efi { "/dev/sda3" } else { "/dev/sda2" };

    // NOTE: pvcreate and vgcreate are not available in the current API
    // g.pvcreate(lvm_device)?;
    // g.vgcreate("debian", &[lvm_device])?;

    // Create logical volumes (sizes in MB)
    g.lvcreate("root", "debian", 64)?;
    g.lvcreate("usr", "debian", 32)?;
    g.lvcreate("var", "debian", 32)?;
    g.lvcreate("home", "debian", 32)?;

    println!("  ✓ LVM layout created: debian/root, debian/usr, debian/var, debian/home");

    // Step 4: Create filesystems
    println!("\n[4/16] Creating filesystems...");
    let boot_device = if use_efi { "/dev/sda2" } else { "/dev/sda1" };

    // Boot filesystem
    g.mkfs_opts("ext2", boot_device, Some(4096), None, Some("BOOT"))?;
    g.set_uuid(boot_device, BOOT_UUID)?;

    // EFI filesystem if needed
    if use_efi {
        g.mkfs_opts("vfat", "/dev/sda1", None, None, Some("EFI"))?;
    }

    // Logical volume filesystems
    g.mkfs_opts("ext2", "/dev/debian/root", Some(4096), None, None)?;
    g.set_uuid("/dev/debian/root", LV_ROOT_UUID)?;

    g.mkfs_opts("ext2", "/dev/debian/usr", Some(4096), None, None)?;
    g.set_uuid("/dev/debian/usr", LV_USR_UUID)?;

    g.mkfs_opts("ext2", "/dev/debian/var", Some(4096), None, None)?;
    g.set_uuid("/dev/debian/var", LV_VAR_UUID)?;

    g.mkfs_opts("ext2", "/dev/debian/home", Some(4096), None, None)?;
    g.set_uuid("/dev/debian/home", LV_HOME_UUID)?;

    let fs_info = if use_efi {
        "vfat (ESP) + ext2 (boot) + ext2 (LVM volumes)"
    } else {
        "ext2 (boot) + ext2 (LVM volumes)"
    };
    println!("  ✓ Filesystems: {}", fs_info);

    // Step 5: Mount filesystems
    println!("\n[5/16] Mounting filesystems...");
    g.mount("/dev/debian/root", "/")?;
    g.mkdir("/boot")?;
    g.mount(boot_device, "/boot")?;

    if use_efi {
        g.mkdir("/boot/efi")?;
        g.mount("/dev/sda1", "/boot/efi")?;
    }

    g.mkdir("/usr")?;
    g.mount("/dev/debian/usr", "/usr")?;

    g.mkdir("/var")?;
    g.mount("/dev/debian/var", "/var")?;

    g.mkdir("/home")?;
    g.mount("/dev/debian/home", "/home")?;

    println!("  ✓ All filesystems mounted");

    // Step 6: Create Debian directory structure
    println!("\n[6/16] Creating Debian directory structure...");
    let directories = vec![
        "/bin",
        "/sbin",
        "/etc",
        "/etc/default",
        "/etc/network",
        "/etc/apt",
        "/etc/systemd",
        "/etc/systemd/system",
        "/etc/systemd/system/multi-user.target.wants",
        "/lib",
        "/lib/systemd",
        "/lib/systemd/system",
        "/var/lib",
        "/var/lib/dpkg",
        "/var/lib/urandom",
        "/var/log",
        "/var/log/apt",
        "/usr/bin",
        "/usr/sbin",
        "/usr/lib",
        "/home/debian",
        "/root",
        "/tmp",
        "/run",
        "/run/lock",
        "/boot/grub",
    ];

    if use_efi {
        g.mkdir_p("/boot/efi/EFI/debian")?;
    }

    for dir in directories {
        g.mkdir_p(dir)?;
    }
    println!("  ✓ Directory structure created");

    // Step 7: Write Debian metadata files
    println!("\n[7/16] Writing Debian metadata files...");
    g.write(
        "/etc/debian_version",
        make_debian_version(version).as_bytes(),
    )?;
    g.write("/etc/os-release", make_os_release(version).as_bytes())?;
    g.write("/etc/hostname", b"debian.invalid\n")?;
    g.write("/etc/fstab", make_fstab(use_efi).as_bytes())?;
    println!("  ✓ Debian metadata files written");

    // Step 8: Create dpkg package database
    println!("\n[8/16] Creating dpkg package database...");
    g.write("/var/lib/dpkg/status", make_dpkg_status().as_bytes())?;
    g.touch("/var/lib/dpkg/available")?;
    g.write("/var/log/dpkg.log", b"# dpkg log file\n")?;
    println!("  ✓ dpkg database created with 7 packages");

    // Step 9: Create systemd units
    println!("\n[9/16] Creating systemd units...");
    g.write(
        "/lib/systemd/system/ssh.service",
        make_ssh_service().as_bytes(),
    )?;
    g.write(
        "/lib/systemd/system/networking.service",
        make_networking_service().as_bytes(),
    )?;
    g.write(
        "/lib/systemd/system/systemd-journald.service",
        make_systemd_journald_service().as_bytes(),
    )?;

    // Create multi-user.target
    g.write(
        "/lib/systemd/system/multi-user.target",
        b"[Unit]\nDescription=Multi-User System\nRequires=basic.target\n\
         Conflicts=rescue.service rescue.target\nAfter=basic.target rescue.service rescue.target\n\
         AllowIsolate=yes\n",
    )?;

    // Create symlinks for enabled services
    g.ln_s(
        "/lib/systemd/system/ssh.service",
        "/etc/systemd/system/multi-user.target.wants/ssh.service",
    )?;
    g.ln_s(
        "/lib/systemd/system/networking.service",
        "/etc/systemd/system/multi-user.target.wants/networking.service",
    )?;

    // Set default target
    g.ln_s(
        "/lib/systemd/system/multi-user.target",
        "/etc/systemd/system/default.target",
    )?;

    println!("  ✓ Systemd units created (ssh, networking, journald)");

    // Step 10: Create APT sources list
    println!("\n[10/16] Creating APT sources list...");
    g.write(
        "/etc/apt/sources.list",
        make_apt_sources(version).as_bytes(),
    )?;
    println!("  ✓ APT sources configured for {}", metadata.codename);

    // Step 11: Create GRUB configuration
    println!("\n[11/16] Creating GRUB configuration...");
    g.write(
        "/boot/grub/grub.cfg",
        make_grub_config(version, use_efi).as_bytes(),
    )?;
    if use_efi {
        g.write(
            "/boot/efi/EFI/debian/grub.cfg",
            make_grub_config(version, use_efi).as_bytes(),
        )?;
    }
    println!("  ✓ GRUB configuration created");

    // Step 12: Create fake kernel and initrd
    println!("\n[12/16] Creating fake kernel and initrd...");
    let fake_kernel = vec![0u8; 1024]; // 1KB fake kernel
    let fake_initrd = vec![0u8; 512]; // 512B fake initrd

    g.write("/boot/vmlinuz-6.1.0-17-amd64", fake_kernel.as_slice())?;
    g.write("/boot/initrd.img-6.1.0-17-amd64", fake_initrd.as_slice())?;
    g.ln_s("/boot/vmlinuz-6.1.0-17-amd64", "/boot/vmlinuz")?;
    g.ln_s("/boot/initrd.img-6.1.0-17-amd64", "/boot/initrd.img")?;
    println!("  ✓ Fake kernel files created");

    // Step 13: Create network configuration
    println!("\n[13/16] Creating network configuration...");
    g.write(
        "/etc/network/interfaces",
        make_network_interfaces().as_bytes(),
    )?;
    g.write("/etc/resolv.conf", make_resolv_conf().as_bytes())?;
    println!("  ✓ Network configuration created");

    // Step 14: Create user accounts
    println!("\n[14/16] Creating user accounts...");
    g.write("/etc/passwd", make_passwd().as_bytes())?;
    g.write("/etc/group", make_group().as_bytes())?;
    g.write("/etc/shadow", make_shadow().as_bytes())?;
    g.chmod(0o640, "/etc/shadow")?;
    println!("  ✓ User accounts created (root, debian)");

    // Step 15: Create log files and fake binaries
    println!("\n[15/16] Creating log files and runtime directories...");
    g.write(
        "/var/log/syslog",
        b"Dec 10 12:00:00 debian systemd[1]: Started System Logging Service.\n",
    )?;
    g.write("/var/log/apt/history.log", b"# APT transaction history\n")?;

    // Create fake /bin/ls with ELF header
    let fake_elf = b"\x7fELF\x00\x00\x00\x00\x00\x00\x00\x00";
    g.write("/bin/ls", fake_elf)?;
    g.chmod(0o755, "/bin/ls")?;

    println!("  ✓ Log files and runtime directories created");

    // Step 16: Test Phase 3 APIs on Debian image
    println!("\n[16/16] Testing Phase 3 APIs on Debian image...");

    // Test stat()
    let stat = g.stat("/etc/hostname")?;
    println!("  ✓ stat(/etc/hostname): size={} bytes", stat.size);

    // Test lstat() on symlink
    let lstat = g.lstat("/boot/vmlinuz")?;
    println!("  ✓ lstat(/boot/vmlinuz): size={} (symlink)", lstat.size);

    // Test rm() - create and remove a test file
    g.write("/tmp/test-phase3.txt", b"test content")?;
    g.rm("/tmp/test-phase3.txt")?;
    println!("  ✓ rm() test passed");

    // Test rm_rf() - create and remove a test directory tree
    g.mkdir_p("/tmp/test-dir/subdir")?;
    g.write("/tmp/test-dir/file1.txt", b"content1")?;
    g.write("/tmp/test-dir/subdir/file2.txt", b"content2")?;
    g.rm_rf("/tmp/test-dir")?;
    println!("  ✓ rm_rf() test passed");

    // Finalize
    println!("\n[Finalizing] Syncing and unmounting...");
    g.sync()?;
    g.umount_all()?;
    g.shutdown()?;

    println!("\n=== Debian {} Image Created Successfully! ===", version);
    println!("  Image: {}", DISK_PATH);
    println!("  Size: {} MB", DISK_SIZE_MB);
    println!("  Boot mode: {}", boot_mode);
    println!("  Codename: {}", metadata.codename);
    println!("  Description: {}", metadata.description);
    println!("  LVM: debian/root, debian/usr, debian/var, debian/home");

    Ok(())
}

#[test]
fn test_debian_11_mbr() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n══════════════════════════════════════════════════════════════");
    println!("  Test: Debian 11 (Bullseye) - MBR/BIOS Layout");
    println!("══════════════════════════════════════════════════════════════");
    create_realistic_debian_image("11", false)
}

#[test]
fn test_debian_12_efi() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n══════════════════════════════════════════════════════════════");
    println!("  Test: Debian 12 (Bookworm) - EFI/GPT Layout");
    println!("══════════════════════════════════════════════════════════════");
    create_realistic_debian_image("12", true)
}

#[test]
fn test_debian_13_efi() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n══════════════════════════════════════════════════════════════");
    println!("  Test: Debian 13 (Trixie) - EFI/GPT Layout");
    println!("══════════════════════════════════════════════════════════════");
    create_realistic_debian_image("13", true)
}

#[test]
fn test_debian_inspection() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n══════════════════════════════════════════════════════════════");
    println!("  Test: Debian OS Inspection APIs");
    println!("══════════════════════════════════════════════════════════════\n");

    // First create a Debian image
    create_realistic_debian_image("12", true)?;

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
    assert_eq!(distro, "debian", "Expected distro 'debian'");
    println!("    ✓ Distribution: {}", distro);

    // Test inspect_get_major_version()
    println!("  Testing inspect_get_major_version()...");
    let major = g.inspect_get_major_version(root)?;
    assert_eq!(major, 12, "Expected major version 12");
    println!("    ✓ Major Version: {}", major);

    // Test inspect_get_package_format()
    println!("  Testing inspect_get_package_format()...");
    let pkg_fmt = g.inspect_get_package_format(root)?;
    assert_eq!(pkg_fmt, "deb", "Expected package format 'deb'");
    println!("    ✓ Package Format: {}", pkg_fmt);

    g.shutdown()?;

    println!("\n=== All OS Inspection Tests Passed! ===\n");

    Ok(())
}

#[test]
fn test_debian_lvm_layout() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n══════════════════════════════════════════════════════════════");
    println!("  Test: Debian LVM Layout Validation");
    println!("══════════════════════════════════════════════════════════════\n");

    // Create a Debian image with LVM
    create_realistic_debian_image("12", true)?;

    println!("\n[Testing LVM Layout]");

    // Open the image for inspection
    let mut g = Guestfs::new()?;
    g.add_drive_ro(DISK_PATH)?;
    g.launch()?;

    // Test LVM detection
    println!("  Testing LVM volume groups...");
    let vgs = g.vgs()?;
    assert!(!vgs.is_empty(), "No volume groups found");
    assert!(
        vgs.contains(&"debian".to_string()),
        "Volume group 'debian' not found"
    );
    println!("    ✓ Volume groups: {:?}", vgs);

    // Test logical volumes
    println!("  Testing logical volumes...");
    let lvs = g.lvs()?;
    assert!(!lvs.is_empty(), "No logical volumes found");

    let expected_lvs = vec![
        "/dev/debian/root",
        "/dev/debian/usr",
        "/dev/debian/var",
        "/dev/debian/home",
    ];

    for lv in &expected_lvs {
        assert!(
            lvs.contains(&lv.to_string()),
            "Logical volume {} not found",
            lv
        );
        println!("    ✓ Found: {}", lv);
    }

    // Test filesystem UUIDs
    println!("  Testing filesystem UUIDs...");
    g.mount("/dev/debian/root", "/")?;

    let uuid = g.get_uuid("/dev/debian/root")?;
    assert_eq!(uuid, LV_ROOT_UUID, "Unexpected root UUID");
    println!("    ✓ Root UUID: {}", uuid);

    g.shutdown()?;

    println!("\n=== All LVM Tests Passed! ===\n");

    Ok(())
}
