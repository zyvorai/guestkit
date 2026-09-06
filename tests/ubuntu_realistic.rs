// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Realistic Ubuntu disk image testing
//!
//! This test creates production-quality Ubuntu disk images with:
//! - GPT partitioning with EFI System Partition
//! - Proper filesystem types per Ubuntu version
//! - Complete systemd unit files
//! - Ubuntu metadata (lsb-release, os-release)
//! - dpkg package database
//! - Realistic directory structure
//! - EFI boot configuration

use guestkit::Guestfs;
use std::collections::HashMap;
use std::fs;

const DISK_PATH: &str = "/tmp/ubuntu-efi-test.img";
const DISK_SIZE: i64 = 2 * 1024 * 1024 * 1024; // 2 GB
const EFI_PART_GUID: &str = "c12a7328-f81f-11d2-ba4b-00a0c93ec93b"; // UEFI spec ESP type GUID

/// Ubuntu version metadata
struct UbuntuVersion {
    codename: &'static str,
    description: &'static str,
    root_fs: &'static str,
}

fn ubuntu_presets() -> HashMap<&'static str, UbuntuVersion> {
    let mut presets = HashMap::new();

    presets.insert(
        "10.10",
        UbuntuVersion {
            codename: "maverick",
            description: "Ubuntu 10.10 (Maverick Meerkat)",
            root_fs: "ext2",
        },
    );

    presets.insert(
        "20.04",
        UbuntuVersion {
            codename: "focal",
            description: "Ubuntu 20.04 LTS (Focal Fossa)",
            root_fs: "ext4",
        },
    );

    presets.insert(
        "22.04",
        UbuntuVersion {
            codename: "jammy",
            description: "Ubuntu 22.04 LTS (Jammy Jellyfish)",
            root_fs: "ext4",
        },
    );

    presets.insert(
        "24.04",
        UbuntuVersion {
            codename: "noble",
            description: "Ubuntu 24.04 LTS (Noble Numbat)",
            root_fs: "xfs",
        },
    );

    presets
}

fn make_lsb_release(version: &str) -> String {
    let presets = ubuntu_presets();
    let default = UbuntuVersion {
        codename: "unknown",
        description: "Ubuntu (unknown version)",
        root_fs: "ext4",
    };
    let metadata = presets.get(version).unwrap_or(&default);

    format!(
        "DISTRIB_ID=Ubuntu\n\
         DISTRIB_RELEASE={}\n\
         DISTRIB_CODENAME={}\n\
         DISTRIB_DESCRIPTION=\"{}\"\n",
        version, metadata.codename, metadata.description
    )
}

fn make_os_release(version: &str) -> String {
    let presets = ubuntu_presets();
    let default = UbuntuVersion {
        codename: "unknown",
        description: "Ubuntu (unknown version)",
        root_fs: "ext4",
    };
    let metadata = presets.get(version).unwrap_or(&default);

    format!(
        "NAME=\"Ubuntu\"\n\
         VERSION=\"{}\"\n\
         ID=ubuntu\n\
         VERSION_ID=\"{}\"\n\
         VERSION_CODENAME={}\n\
         ID_LIKE=debian\n\
         PRETTY_NAME=\"{}\"\n\
         HOME_URL=\"https://www.ubuntu.com/\"\n\
         SUPPORT_URL=\"https://help.ubuntu.com/\"\n\
         BUG_REPORT_URL=\"https://bugs.launchpad.net/ubuntu/\"\n\
         PRIVACY_POLICY_URL=\"https://www.ubuntu.com/legal/terms-and-policies/privacy-policy\"\n\
         UBUNTU_CODENAME={}\n",
        metadata.description, version, metadata.codename, metadata.description, metadata.codename
    )
}

fn make_fstab(root_fs: &str) -> String {
    format!(
        "# /etc/fstab: static file system information.\n\
         #\n\
         # Use 'blkid' to print the universally unique identifier for a\n\
         # device; this may be used with UUID= as a more robust way to name devices\n\
         # that works even if disks are added and removed. See fstab(5).\n\
         #\n\
         # <file system> <mount point>   <type>  <options>       <dump>  <pass>\n\
         /dev/sda2       /               {}      defaults        1       1\n\
         /dev/sda1       /boot/efi       vfat    umask=0077      0       1\n\
         \n\
         # Dummy encrypted swap device\n\
         /dev/mapper/cryptswap1  none    swap    sw              0       0\n",
        root_fs
    )
}

fn make_dpkg_status() -> String {
    "Package: base-files\n\
     Status: install ok installed\n\
     Priority: required\n\
     Section: admin\n\
     Installed-Size: 384\n\
     Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>\n\
     Architecture: amd64\n\
     Version: 12ubuntu4.6\n\
     Description: Debian base system miscellaneous files\n\
     \n\
     Package: bash\n\
     Status: install ok installed\n\
     Priority: required\n\
     Section: shells\n\
     Installed-Size: 1789\n\
     Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>\n\
     Architecture: amd64\n\
     Version: 5.1-6ubuntu1.1\n\
     Description: GNU Bourne Again SHell\n\
     \n\
     Package: coreutils\n\
     Status: install ok installed\n\
     Priority: required\n\
     Section: utils\n\
     Installed-Size: 14824\n\
     Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>\n\
     Architecture: amd64\n\
     Version: 8.32-4.1ubuntu1.2\n\
     Description: GNU core utilities\n\
     \n\
     Package: dpkg\n\
     Status: install ok installed\n\
     Priority: required\n\
     Section: admin\n\
     Installed-Size: 6432\n\
     Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>\n\
     Architecture: amd64\n\
     Version: 1.21.1ubuntu2.3\n\
     Description: Debian package management system\n\
     \n\
     Package: openssh-server\n\
     Status: install ok installed\n\
     Priority: optional\n\
     Section: net\n\
     Installed-Size: 1328\n\
     Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>\n\
     Architecture: amd64\n\
     Version: 1:8.9p1-3ubuntu0.6\n\
     Description: secure shell (SSH) server, for secure access from remote machines\n\
     \n\
     Package: systemd\n\
     Status: install ok installed\n\
     Priority: important\n\
     Section: admin\n\
     Installed-Size: 18432\n\
     Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>\n\
     Architecture: amd64\n\
     Version: 249.11-0ubuntu3.12\n\
     Description: system and service manager\n\
     \n\
     Package: linux-image-6.5.0-35-generic\n\
     Status: install ok installed\n\
     Priority: optional\n\
     Section: kernel\n\
     Installed-Size: 14256\n\
     Maintainer: Ubuntu Kernel Team <kernel-team@lists.ubuntu.com>\n\
     Architecture: amd64\n\
     Version: 6.5.0-35.35~22.04.1\n\
     Description: Linux kernel image for version 6.5.0 on 64 bit x86 SMP\n\
     \n"
    .to_string()
}

fn make_ssh_service() -> String {
    "[Unit]\n\
     Description=OpenBSD Secure Shell server\n\
     Documentation=man:sshd(8) man:sshd_config(5)\n\
     After=network.target auditd.service\n\
     ConditionPathExists=!/etc/ssh/sshd_not_to_be_run\n\
     \n\
     [Service]\n\
     EnvironmentFile=-/etc/default/ssh\n\
     ExecStartPre=/usr/sbin/sshd -t\n\
     ExecStart=/usr/sbin/sshd -D $SSHD_OPTS\n\
     ExecReload=/usr/sbin/sshd -t\n\
     ExecReload=/bin/kill -HUP $MAINPID\n\
     KillMode=process\n\
     Restart=on-failure\n\
     RestartPreventExitStatus=255\n\
     Type=notify\n\
     RuntimeDirectory=sshd\n\
     RuntimeDirectoryMode=0755\n\
     \n\
     [Install]\n\
     WantedBy=multi-user.target\n\
     Alias=sshd.service\n"
        .to_string()
}

fn make_networking_service() -> String {
    "[Unit]\n\
     Description=Raise network interfaces\n\
     Documentation=man:interfaces(5)\n\
     DefaultDependencies=no\n\
     Wants=network.target\n\
     After=local-fs.target network-pre.target apparmor.service systemd-sysctl.service systemd-modules-load.service\n\
     Before=network.target shutdown.target network-online.target\n\
     Conflicts=shutdown.target\n\
     \n\
     [Service]\n\
     Type=oneshot\n\
     EnvironmentFile=-/etc/default/networking\n\
     ExecStart=/sbin/ifup -a --read-environment\n\
     ExecStop=/sbin/ifdown -a --read-environment --exclude=lo\n\
     RemainAfterExit=true\n\
     TimeoutStartSec=5min\n\
     \n\
     [Install]\n\
     WantedBy=multi-user.target\n\
     WantedBy=network-online.target\n".to_string()
}

fn make_systemd_journald_service() -> String {
    "[Unit]\n\
     Description=Journal Service\n\
     Documentation=man:systemd-journald.service(8) man:journald.conf(5)\n\
     DefaultDependencies=no\n\
     Requires=systemd-journald.socket\n\
     After=systemd-journald.socket systemd-journald-dev-log.socket systemd-journald-audit.socket syslog.socket\n\
     Before=sysinit.target\n\
     \n\
     [Service]\n\
     Type=notify\n\
     Sockets=systemd-journald.socket systemd-journald-dev-log.socket systemd-journald-audit.socket\n\
     ExecStart=/lib/systemd/systemd-journald\n\
     Restart=always\n\
     RestartSec=0\n\
     StandardOutput=null\n\
     CapabilityBoundingSet=CAP_SYS_ADMIN CAP_DAC_OVERRIDE CAP_SYS_PTRACE CAP_SYSLOG CAP_AUDIT_CONTROL CAP_AUDIT_READ CAP_CHOWN CAP_DAC_READ_SEARCH CAP_FOWNER CAP_SETUID CAP_SETGID CAP_MAC_OVERRIDE\n\
     WatchdogSec=3min\n\
     \n\
     # Increase the default a bit in order to allow many simultaneous\n\
     # services being run since we keep one fd open per service. Also, when\n\
     # flushing journal files to disk, we might need a lot of fds when many\n\
     # journal files are active.\n\
     LimitNOFILE=524288\n".to_string()
}

fn make_grub_config() -> String {
    "# GRUB configuration for Ubuntu\n\
     #\n\
     # This is a fake/minimal configuration for testing\n\
     \n\
     set timeout=5\n\
     set default=0\n\
     \n\
     menuentry 'Ubuntu' {\n\
         set root='hd0,gpt2'\n\
         linux /boot/vmlinuz-6.5.0-35-generic root=/dev/sda2 ro quiet splash\n\
         initrd /boot/initrd.img-6.5.0-35-generic\n\
     }\n\
     \n\
     menuentry 'Ubuntu (recovery mode)' {\n\
         set root='hd0,gpt2'\n\
         linux /boot/vmlinuz-6.5.0-35-generic root=/dev/sda2 ro recovery nomodeset\n\
         initrd /boot/initrd.img-6.5.0-35-generic\n\
     }\n"
    .to_string()
}

fn make_apt_sources(codename: &str) -> String {
    format!(
        "# Ubuntu {} repositories\n\
         \n\
         deb http://archive.ubuntu.com/ubuntu/ {} main restricted\n\
         deb http://archive.ubuntu.com/ubuntu/ {} universe\n\
         deb http://archive.ubuntu.com/ubuntu/ {} multiverse\n\
         \n\
         deb http://archive.ubuntu.com/ubuntu/ {}-updates main restricted\n\
         deb http://archive.ubuntu.com/ubuntu/ {}-updates universe\n\
         deb http://archive.ubuntu.com/ubuntu/ {}-updates multiverse\n\
         \n\
         deb http://archive.ubuntu.com/ubuntu/ {}-backports main restricted universe multiverse\n\
         \n\
         deb http://security.ubuntu.com/ubuntu/ {}-security main restricted\n\
         deb http://security.ubuntu.com/ubuntu/ {}-security universe\n\
         deb http://security.ubuntu.com/ubuntu/ {}-security multiverse\n",
        codename,
        codename,
        codename,
        codename,
        codename,
        codename,
        codename,
        codename,
        codename,
        codename,
        codename
    )
}

fn cleanup() {
    let _ = fs::remove_file(DISK_PATH);
}

fn create_realistic_ubuntu_image(version: &str) -> Result<(), Box<dyn std::error::Error>> {
    cleanup();

    println!(
        "\n=== Creating Realistic Ubuntu {} EFI Disk Image ===",
        version
    );

    let presets = ubuntu_presets();
    let metadata = presets.get(version).unwrap_or(&UbuntuVersion {
        codename: "jammy",
        description: "Ubuntu 22.04 LTS (Jammy Jellyfish)",
        root_fs: "ext4",
    });

    println!("\n[1/15] Creating Guestfs handle and disk image...");
    let mut g = Guestfs::create()?;
    g.set_verbose(false);

    g.disk_create(DISK_PATH, "raw", DISK_SIZE)?;
    g.add_drive(DISK_PATH)?;
    g.launch()?;
    println!("  ✓ Disk image created and guestfs launched");

    println!("\n[2/15] Creating GPT partition table with EFI System Partition...");
    g.part_init("/dev/sda", "gpt")?;

    // 200MB ESP: sectors 2048 to 411647
    g.part_add("/dev/sda", "p", 2048, 411647)?;

    // Root: rest of disk, leaving GPT metadata at end
    g.part_add("/dev/sda", "p", 411648, -34)?;

    // Mark partition 1 as EFI System Partition
    g.part_set_gpt_type("/dev/sda", 1, EFI_PART_GUID)?;
    println!("  ✓ GPT with EFI System Partition created");

    println!("\n[3/15] Creating filesystems...");
    // Format ESP as FAT32
    g.mkfs("vfat", "/dev/sda1")?;

    // Format root with version-appropriate filesystem
    g.mkfs(metadata.root_fs, "/dev/sda2")?;
    println!("  ✓ Filesystems: vfat (ESP) + {} (root)", metadata.root_fs);

    println!("\n[4/15] Mounting filesystems...");
    g.mount("/dev/sda2", "/")?;
    g.mkdir_p("/boot/efi")?;
    g.mount("/dev/sda1", "/boot/efi")?;
    println!("  ✓ Root and EFI partitions mounted");

    println!("\n[5/15] Creating Ubuntu directory structure...");
    // FHS directory structure
    for dir in &[
        "/bin",
        "/sbin",
        "/lib",
        "/lib64",
        "/usr",
        "/usr/bin",
        "/usr/sbin",
        "/usr/lib",
        "/usr/local",
        "/etc",
        "/etc/systemd",
        "/etc/systemd/system",
        "/etc/systemd/system/multi-user.target.wants",
        "/etc/default",
        "/etc/apt",
        "/etc/apt/sources.list.d",
        "/var",
        "/var/lib",
        "/var/lib/dpkg",
        "/var/log",
        "/home",
        "/root",
        "/boot",
        "/boot/grub",
        "/boot/efi/EFI",
        "/boot/efi/EFI/ubuntu",
        "/tmp",
        "/run",
        "/lib/systemd",
        "/lib/systemd/system",
        "/opt",
        "/srv",
        "/mnt",
        "/media",
    ] {
        g.mkdir_p(dir)?;
    }
    println!("  ✓ Directory structure created");

    println!("\n[6/15] Writing Ubuntu metadata files...");
    g.write("/etc/lsb-release", make_lsb_release(version).as_bytes())?;
    g.write("/etc/os-release", make_os_release(version).as_bytes())?;
    g.write("/etc/hostname", b"ubuntu-test.localdomain\n")?;
    g.write("/etc/debian_version", b"bookworm/sid\n")?;
    g.write("/etc/fstab", make_fstab(metadata.root_fs).as_bytes())?;
    println!("  ✓ Ubuntu metadata files written");

    println!("\n[7/15] Creating dpkg package database...");
    g.write("/var/lib/dpkg/status", make_dpkg_status().as_bytes())?;
    g.write("/var/lib/dpkg/available", b"")?;
    println!("  ✓ dpkg database created with {} packages", 7);

    println!("\n[8/15] Creating systemd units...");
    // SSH service
    g.write(
        "/lib/systemd/system/ssh.service",
        make_ssh_service().as_bytes(),
    )?;
    g.ln_s(
        "../../../lib/systemd/system/ssh.service",
        "/etc/systemd/system/multi-user.target.wants/ssh.service",
    )?;

    // Networking service
    g.write(
        "/lib/systemd/system/networking.service",
        make_networking_service().as_bytes(),
    )?;
    g.ln_s(
        "../../../lib/systemd/system/networking.service",
        "/etc/systemd/system/multi-user.target.wants/networking.service",
    )?;

    // Journal service
    g.write(
        "/lib/systemd/system/systemd-journald.service",
        make_systemd_journald_service().as_bytes(),
    )?;

    // Multi-user target
    g.write("/lib/systemd/system/multi-user.target",
            b"[Unit]\nDescription=Multi-User System\nDocumentation=man:systemd.special(7)\nRequires=basic.target\nConflicts=rescue.service rescue.target\nAfter=basic.target rescue.service rescue.target\nAllowIsolate=yes\n")?;
    g.ln_s(
        "/lib/systemd/system/multi-user.target",
        "/etc/systemd/system/default.target",
    )?;

    println!("  ✓ Systemd units created (ssh, networking, journald)");

    println!("\n[9/15] Creating APT sources list...");
    g.write(
        "/etc/apt/sources.list",
        make_apt_sources(metadata.codename).as_bytes(),
    )?;
    println!("  ✓ APT sources configured for {}", metadata.codename);

    println!("\n[10/15] Creating GRUB configuration...");
    g.write("/boot/grub/grub.cfg", make_grub_config().as_bytes())?;
    g.write("/boot/efi/EFI/ubuntu/grub.cfg",
            b"# EFI GRUB configuration\nsearch.fs_uuid 1234-5678 root\nset prefix=($root)/boot/grub\nconfigfile $prefix/grub.cfg\n")?;
    println!("  ✓ GRUB configuration created");

    println!("\n[11/15] Creating fake kernel and initrd...");
    g.write("/boot/vmlinuz-6.5.0-35-generic", b"FAKE_KERNEL_BINARY_DATA")?;
    g.write("/boot/initrd.img-6.5.0-35-generic", b"FAKE_INITRD_DATA")?;
    g.ln_s("vmlinuz-6.5.0-35-generic", "/boot/vmlinuz")?;
    g.ln_s("initrd.img-6.5.0-35-generic", "/boot/initrd.img")?;
    println!("  ✓ Fake kernel files created");

    println!("\n[12/15] Creating network configuration...");
    g.write(
        "/etc/network/interfaces",
        b"# interfaces(5) file used by ifup(8) and ifdown(8)\n\
              auto lo\n\
              iface lo inet loopback\n\
              \n\
              auto enp0s3\n\
              iface enp0s3 inet dhcp\n",
    )?;
    g.write(
        "/etc/resolv.conf",
        b"nameserver 8.8.8.8\nnameserver 8.8.4.4\n",
    )?;
    println!("  ✓ Network configuration created");

    println!("\n[13/15] Creating user accounts...");
    g.write(
        "/etc/passwd",
        b"root:x:0:0:root:/root:/bin/bash\n\
              daemon:x:1:1:daemon:/usr/sbin:/usr/sbin/nologin\n\
              bin:x:2:2:bin:/bin:/usr/sbin/nologin\n\
              sys:x:3:3:sys:/dev:/usr/sbin/nologin\n\
              ubuntu:x:1000:1000:Ubuntu User:/home/ubuntu:/bin/bash\n",
    )?;
    g.write(
        "/etc/group",
        b"root:x:0:\n\
              daemon:x:1:\n\
              bin:x:2:\n\
              sys:x:3:\n\
              sudo:x:27:ubuntu\n\
              ubuntu:x:1000:\n",
    )?;
    g.write(
        "/etc/shadow",
        b"root:!:19000:0:99999:7:::\n\
              daemon:*:19000:0:99999:7:::\n\
              bin:*:19000:0:99999:7:::\n\
              sys:*:19000:0:99999:7:::\n\
              ubuntu:!:19000:0:99999:7:::\n",
    )?;
    g.mkdir_p("/home/ubuntu")?;
    println!("  ✓ User accounts created (root, ubuntu)");

    println!("\n[14/15] Creating log files and runtime directories...");
    g.touch("/var/log/dpkg.log")?;
    g.touch("/var/log/apt/history.log")?;
    g.touch("/var/log/syslog")?;
    g.mkdir_p("/run/systemd")?;
    g.mkdir_p("/run/lock")?;
    println!("  ✓ Log files and runtime directories created");

    println!("\n[15/15] Testing Phase 3 APIs on Ubuntu image...");

    // Test stat()
    let stat = g.stat("/etc/hostname")?;
    println!("  ✓ stat(/etc/hostname): size={} bytes", stat.size);

    // Test lstat() on symlink
    let lstat = g.lstat("/boot/vmlinuz")?;
    println!("  ✓ lstat(/boot/vmlinuz): size={} (symlink)", lstat.size);

    // Test file operations
    g.write("/tmp/test-phase3.txt", b"Phase 3 test data\n")?;
    assert!(g.exists("/tmp/test-phase3.txt")?);
    g.rm("/tmp/test-phase3.txt")?;
    assert!(!g.exists("/tmp/test-phase3.txt")?);
    println!("  ✓ rm() test passed");

    // Test rm_rf()
    g.mkdir_p("/tmp/test-dir/subdir")?;
    g.write("/tmp/test-dir/file.txt", b"test")?;
    g.rm_rf("/tmp/test-dir")?;
    assert!(!g.exists("/tmp/test-dir")?);
    println!("  ✓ rm_rf() test passed");

    println!("\n[Finalizing] Syncing and unmounting...");
    g.sync()?;
    g.umount_all()?;
    g.shutdown()?;

    println!("\n=== Ubuntu {} Image Created Successfully! ===", version);
    println!("  Image: {}", DISK_PATH);
    println!(
        "  Size: {:.2} GB",
        DISK_SIZE as f64 / 1024.0 / 1024.0 / 1024.0
    );
    println!("  Filesystem: {}", metadata.root_fs);
    println!("  Codename: {}", metadata.codename);
    println!("  Description: {}", metadata.description);

    Ok(())
}

#[test]
fn test_ubuntu_2204_realistic() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Testing Ubuntu 22.04 LTS (Jammy Jellyfish) ===");
    let result = create_realistic_ubuntu_image("22.04");
    cleanup();
    result
}

#[test]
fn test_ubuntu_2004_realistic() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Testing Ubuntu 20.04 LTS (Focal Fossa) ===");
    let result = create_realistic_ubuntu_image("20.04");
    cleanup();
    result
}

#[test]
fn test_ubuntu_2404_realistic() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Testing Ubuntu 24.04 LTS (Noble Numbat) ===");
    let result = create_realistic_ubuntu_image("24.04");
    cleanup();
    result
}

#[test]
fn test_ubuntu_inspection() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Testing Ubuntu OS Inspection ===");

    // Create Ubuntu image
    create_realistic_ubuntu_image("22.04")?;

    // Re-open for inspection
    let mut g = Guestfs::new()?;
    g.add_drive_ro(DISK_PATH)?;
    g.launch()?;

    println!("\n[1/5] Inspecting operating system...");
    let roots = g.inspect_os()?;
    assert!(!roots.is_empty(), "Should detect Ubuntu OS");

    let root = &roots[0];
    println!("  ✓ OS detected: {}", root);

    println!("\n[2/5] Checking OS type...");
    let ostype = g.inspect_get_type(root)?;
    assert_eq!(ostype, "linux");
    println!("  ✓ OS type: {}", ostype);

    println!("\n[3/5] Checking distribution...");
    let distro = g.inspect_get_distro(root)?;
    assert_eq!(distro, "ubuntu");
    println!("  ✓ Distribution: {}", distro);

    println!("\n[4/5] Checking version...");
    let major = g.inspect_get_major_version(root)?;
    let minor = g.inspect_get_minor_version(root)?;
    println!("  ✓ Version: {}.{}", major, minor);

    println!("\n[5/5] Checking package format...");
    let pkg_fmt = g.inspect_get_package_format(root)?;
    assert_eq!(pkg_fmt, "deb");
    println!("  ✓ Package format: {}", pkg_fmt);

    g.shutdown()?;
    cleanup();

    println!("\n✓ All Ubuntu inspection tests passed!");
    Ok(())
}
