// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Realistic Windows disk image testing
//!
//! This test creates production-quality Windows disk images with:
//! - Both MBR and GPT partitioning
//! - NTFS filesystem
//! - Complete Windows directory structure
//! - Windows registry simulation
//! - Windows services
//! - Multiple Windows versions (10, 11, Server 2022)

use guestkit::Guestfs;
use std::collections::HashMap;
use std::fs;

const DISK_PATH: &str = "/tmp/windows-test.img";
const DISK_SIZE_MB: i64 = 1024; // 1 GB
const EFI_PART_GUID: &str = "c12a7328-f81f-11d2-ba4b-00a0c93ec93b";
const WINDOWS_RESERVED_GUID: &str = "e3c9e316-0b5c-4db8-817d-f92df00215ae"; // Microsoft Reserved (MSR)

/// Windows version metadata
struct WindowsVersion {
    product_name: &'static str,
    version: &'static str,
    build: &'static str,
    edition: &'static str,
}

fn windows_versions() -> HashMap<&'static str, WindowsVersion> {
    let mut versions = HashMap::new();

    versions.insert(
        "10",
        WindowsVersion {
            product_name: "Windows 10 Pro",
            version: "10.0",
            build: "19045",
            edition: "Professional",
        },
    );

    versions.insert(
        "11",
        WindowsVersion {
            product_name: "Windows 11 Pro",
            version: "10.0",
            build: "22631",
            edition: "Professional",
        },
    );

    versions.insert(
        "server2022",
        WindowsVersion {
            product_name: "Windows Server 2022",
            version: "10.0",
            build: "20348",
            edition: "ServerStandard",
        },
    );

    versions
}

fn make_system_registry(version_meta: &WindowsVersion) -> String {
    format!(
        r#"Windows Registry Editor Version 5.00

[HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows NT\CurrentVersion]
"ProductName"="{}"
"EditionID"="{}"
"CurrentVersion"="{}"
"CurrentBuild"="{}"
"CurrentBuildNumber"="{}"
"InstallationType"="Client"
"RegisteredOwner"="Windows User"
"RegisteredOrganization"=""
"SystemRoot"="C:\\Windows"
"PathName"="C:\\Windows"

[HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\ComputerName\ComputerName]
"ComputerName"="WIN-TEST"

[HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters]
"Hostname"="WIN-TEST"
"Domain"=""
"DhcpNameServer"="8.8.8.8 8.8.4.4"
"#,
        version_meta.product_name,
        version_meta.edition,
        version_meta.version,
        version_meta.build,
        version_meta.build
    )
}

fn make_software_registry() -> String {
    r#"Windows Registry Editor Version 5.00

[HKEY_LOCAL_MACHINE\SOFTWARE\Classes]

[HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall]

[HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths]

[HKEY_LOCAL_MACHINE\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate]
"#
    .to_string()
}

fn make_boot_bcd() -> String {
    r#"Windows Boot Manager
--------------------
identifier              {bootmgr}
device                  partition=\Device\HarddiskVolume1
description             Windows Boot Manager
locale                  en-US
inherit                 {globalsettings}
default                 {current}
resumeobject            {current}
displayorder            {current}
toolsdisplayorder       {memdiag}
timeout                 30

Windows Boot Loader
-------------------
identifier              {current}
device                  partition=C:
path                    \Windows\system32\winload.efi
description             Windows 11
locale                  en-US
inherit                 {bootloadersettings}
recoverysequence        {recovery}
recoveryenabled         Yes
isolatedcontext         Yes
allowedinmemorysettings 0x15000075
osdevice                partition=C:
systemroot              \Windows
resumeobject            {current}
nx                      OptIn
bootmenupolicy          Standard
"#
    .to_string()
}

fn make_unattend_xml(version_meta: &WindowsVersion) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<unattend xmlns="urn:schemas-microsoft-com:unattend">
    <settings pass="windowsPE">
        <component name="Microsoft-Windows-Setup" processorArchitecture="amd64" language="neutral">
            <ImageInstall>
                <OSImage>
                    <InstallFrom>
                        <MetaData wcm:action="add">
                            <Key>/IMAGE/NAME</Key>
                            <Value>{}</Value>
                        </MetaData>
                    </InstallFrom>
                </OSImage>
            </ImageInstall>
        </component>
    </settings>
    <settings pass="specialize">
        <component name="Microsoft-Windows-Shell-Setup" processorArchitecture="amd64" language="neutral">
            <ComputerName>WIN-TEST</ComputerName>
            <RegisteredOrganization>Test Organization</RegisteredOrganization>
            <RegisteredOwner>Test User</RegisteredOwner>
        </component>
    </settings>
</unattend>
"#,
        version_meta.product_name
    )
}

fn make_hosts_file() -> String {
    r#"# Copyright (c) 1993-2009 Microsoft Corp.
#
# This is a sample HOSTS file used by Microsoft TCP/IP for Windows.
#
# This file contains the mappings of IP addresses to host names. Each
# entry should be kept on an individual line. The IP address should
# be placed in the first column followed by the corresponding host name.
# The IP address and the host name should be separated by at least one
# space.
#
# Additionally, comments (such as these) may be inserted on individual
# lines or following the machine name denoted by a '#' symbol.
#
# For example:
#
#      102.54.94.97     rhino.acme.com          # source server
#       38.25.63.10     x.acme.com              # x client host

# localhost name resolution is handled within DNS itself.
#	127.0.0.1       localhost
#	::1             localhost
"#
    .to_string()
}

fn make_windows_services() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "wuauserv",
            r#"[Service]
Type=own
Start=demand
ErrorControl=normal
ImagePath=%systemroot%\system32\svchost.exe -k netsvcs -p
DisplayName=Windows Update
Description=Enables the detection, download, and installation of updates for Windows and other programs.
DependOnService=rpcss
ObjectName=LocalSystem
"#,
        ),
        (
            "Dhcp",
            r#"[Service]
Type=share
Start=auto
ErrorControl=normal
ImagePath=%SystemRoot%\system32\svchost.exe -k NetworkService -p
DisplayName=DHCP Client
Description=Registers and updates IP addresses and DNS records for this computer.
DependOnService=Tcpip/Afd/NetBT
ObjectName=NT AUTHORITY\LocalService
"#,
        ),
        (
            "Dnscache",
            r#"[Service]
Type=share
Start=auto
ErrorControl=normal
ImagePath=%SystemRoot%\system32\svchost.exe -k NetworkService -p
DisplayName=DNS Client
Description=The DNS Client service (dnscache) caches Domain Name System (DNS) names and registers the full computer name for this computer.
DependOnService=Tcpip
ObjectName=NT AUTHORITY\NetworkService
"#,
        ),
        (
            "EventLog",
            r#"[Service]
Type=share
Start=auto
ErrorControl=normal
ImagePath=%SystemRoot%\System32\svchost.exe -k LocalServiceNetworkRestricted -p
DisplayName=Windows Event Log
Description=This service manages events and event logs.
ObjectName=NT AUTHORITY\LocalService
"#,
        ),
    ]
}

fn create_realistic_windows_image(
    version: &str,
    use_efi: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let versions = windows_versions();
    let version_meta = versions.get(version).ok_or("Unknown Windows version")?;

    let boot_mode = if use_efi { "EFI/GPT" } else { "BIOS/MBR" };

    println!(
        "\n=== Creating Realistic {} Image ({}) ===\n",
        version_meta.product_name, boot_mode
    );

    // Step 1: Create disk image and guestfs handle
    println!("[1/16] Creating Guestfs handle and disk image...");
    let mut g = Guestfs::create()?;

    // Remove old image if it exists
    let _ = fs::remove_file(DISK_PATH);

    // Create sparse image
    let disk_size_bytes = DISK_SIZE_MB * 1024 * 1024;
    g.disk_create(DISK_PATH, "raw", disk_size_bytes)?;
    g.add_drive(DISK_PATH)?;
    g.launch()?;
    println!("  ✓ Disk image created and guestfs launched");

    // Step 2: Create partition table
    if use_efi {
        println!("\n[2/16] Creating GPT partition table for UEFI...");
        g.part_init("/dev/sda", "gpt")?;

        // EFI System Partition: 100MB
        let esp_start = 2048;
        let esp_end = esp_start + ((100 * 1024 * 1024) / 512) - 1;
        g.part_add("/dev/sda", "p", esp_start, esp_end)?;
        g.part_set_gpt_type("/dev/sda", 1, EFI_PART_GUID)?;
        g.part_set_name("/dev/sda", 1, "EFI system partition")?;

        // Microsoft Reserved: 16MB
        let msr_start = esp_end + 1;
        let msr_end = msr_start + ((16 * 1024 * 1024) / 512) - 1;
        g.part_add("/dev/sda", "p", msr_start, msr_end)?;
        g.part_set_gpt_type("/dev/sda", 2, WINDOWS_RESERVED_GUID)?;
        g.part_set_name("/dev/sda", 2, "Microsoft reserved partition")?;

        // Windows partition: rest of disk
        let win_start = msr_end + 1;
        g.part_add("/dev/sda", "p", win_start, -34)?;
        g.part_set_name("/dev/sda", 3, "Windows")?;

        println!("  ✓ GPT with UEFI partitions created");
    } else {
        println!("\n[2/16] Creating MBR partition table for BIOS...");
        g.part_init("/dev/sda", "mbr")?;

        // System Reserved: 100MB
        g.part_add("/dev/sda", "p", 2048, 206847)?;
        g.part_set_mbr_id("/dev/sda", 1, 0x07)?; // NTFS

        // Windows partition: rest of disk
        g.part_add("/dev/sda", "p", 206848, -2048)?;
        g.part_set_mbr_id("/dev/sda", 2, 0x07)?; // NTFS
        g.part_set_bootable("/dev/sda", 1, true)?;

        println!("  ✓ MBR partition table created");
    }

    // Step 3: Create filesystems
    println!("\n[3/16] Creating filesystems...");
    if use_efi {
        g.mkfs("vfat", "/dev/sda1")?;
        g.mkfs("ntfs", "/dev/sda3")?;
        println!("  ✓ Filesystems: vfat (ESP) + NTFS (Windows)");
    } else {
        g.mkfs("ntfs", "/dev/sda1")?;
        g.mkfs("ntfs", "/dev/sda2")?;
        println!("  ✓ Filesystems: NTFS (System Reserved) + NTFS (Windows)");
    }

    // Step 4: Mount filesystems
    println!("\n[4/16] Mounting filesystems...");
    let windows_part = if use_efi { "/dev/sda3" } else { "/dev/sda2" };
    g.mount(windows_part, "/")?;
    println!("  ✓ Windows partition mounted");

    // Step 5: Create Windows directory structure
    println!("\n[5/16] Creating Windows directory structure...");
    let directories = vec![
        "/Windows",
        "/Windows/System32",
        "/Windows/System32/config",
        "/Windows/System32/drivers",
        "/Windows/System32/drivers/etc",
        "/Windows/System32/winevt",
        "/Windows/System32/winevt/Logs",
        "/Windows/SysWOW64",
        "/Windows/Boot",
        "/Windows/Boot/EFI",
        "/Windows/Temp",
        "/Windows/Logs",
        "/Windows/Logs/CBS",
        "/Windows/Panther",
        "/Program Files",
        "/Program Files/Common Files",
        "/Program Files/Windows Defender",
        "/Program Files (x86)",
        "/ProgramData",
        "/ProgramData/Microsoft",
        "/ProgramData/Microsoft/Windows",
        "/Users",
        "/Users/Administrator",
        "/Users/Administrator/Desktop",
        "/Users/Administrator/Documents",
        "/Users/Administrator/Downloads",
        "/Users/Public",
        "/Temp",
        "/Recovery",
        "/System Volume Information",
    ];

    for dir in directories {
        g.mkdir_p(dir)?;
    }

    if use_efi {
        g.mkdir_p("/EFI/Microsoft/Boot")?;
    }

    println!("  ✓ Directory structure created");

    // Step 6: Create Windows registry files
    println!("\n[6/16] Creating Windows registry files...");
    g.write(
        "/Windows/System32/config/SYSTEM",
        make_system_registry(version_meta).as_bytes(),
    )?;
    g.write(
        "/Windows/System32/config/SOFTWARE",
        make_software_registry().as_bytes(),
    )?;
    g.write("/Windows/System32/config/SAM", b"SAM Registry Hive\n")?;
    g.write(
        "/Windows/System32/config/SECURITY",
        b"SECURITY Registry Hive\n",
    )?;
    g.write(
        "/Windows/System32/config/DEFAULT",
        b"DEFAULT Registry Hive\n",
    )?;
    println!("  ✓ Registry hives created");

    // Step 7: Create boot configuration
    println!("\n[7/16] Creating boot configuration...");
    if use_efi {
        g.write("/Windows/Boot/EFI/BCD", make_boot_bcd().as_bytes())?;
        g.write("/EFI/Microsoft/Boot/BCD", make_boot_bcd().as_bytes())?;
    } else {
        g.write("/Windows/Boot/PCAT/BCD", make_boot_bcd().as_bytes())?;
    }
    println!("  ✓ Boot configuration created");

    // Step 8: Create Windows system files
    println!("\n[8/16] Creating Windows system files...");
    g.write(
        "/Windows/System32/drivers/etc/hosts",
        make_hosts_file().as_bytes(),
    )?;
    g.write(
        "/Windows/Panther/unattend.xml",
        make_unattend_xml(version_meta).as_bytes(),
    )?;

    // Create version info file
    g.write(
        "/Windows/System32/version.txt",
        format!(
            "{}\nVersion {}\nBuild {}\n",
            version_meta.product_name, version_meta.version, version_meta.build
        )
        .as_bytes(),
    )?;

    println!("  ✓ Windows system files created");

    // Step 9: Create Windows services
    println!("\n[9/16] Creating Windows services...");
    for (service_name, service_config) in make_windows_services() {
        let service_path = format!("/Windows/System32/config/services/{}.ini", service_name);
        g.write(&service_path, service_config.as_bytes())?;
    }
    println!("  ✓ Windows services configured");

    // Step 10: Create fake Windows binaries
    println!("\n[10/16] Creating fake Windows binaries...");
    let fake_pe = b"MZ\x90\x00\x03\x00\x00\x00\x04\x00\x00\x00\xFF\xFF\x00\x00"; // Minimal PE header

    let binaries = vec![
        "/Windows/System32/cmd.exe",
        "/Windows/System32/powershell.exe",
        "/Windows/System32/notepad.exe",
        "/Windows/System32/explorer.exe",
        "/Windows/System32/svchost.exe",
        "/Windows/System32/services.exe",
        "/Windows/System32/lsass.exe",
        "/Windows/System32/winlogon.exe",
    ];

    for binary in binaries {
        g.write(binary, fake_pe)?;
    }

    println!("  ✓ Fake Windows binaries created");

    // Step 11: Create event logs
    println!("\n[11/16] Creating event logs...");
    g.write(
        "/Windows/System32/winevt/Logs/System.evtx",
        b"Windows Event Log\n",
    )?;
    g.write(
        "/Windows/System32/winevt/Logs/Application.evtx",
        b"Windows Event Log\n",
    )?;
    g.write(
        "/Windows/System32/winevt/Logs/Security.evtx",
        b"Windows Event Log\n",
    )?;
    println!("  ✓ Event logs created");

    // Step 12: Create user profiles
    println!("\n[12/16] Creating user profiles...");
    g.write("/Users/Administrator/NTUSER.DAT", b"User Registry Hive\n")?;
    g.write(
        "/Users/Administrator/Desktop/desktop.ini",
        b"[.ShellClassInfo]\nLocalizedResourceName=@%SystemRoot%\\system32\\shell32.dll,-21769\n",
    )?;
    println!("  ✓ User profiles created");

    // Step 13: Create Windows Update files
    println!("\n[13/16] Creating Windows Update metadata...");
    g.mkdir_p("/Windows/SoftwareDistribution/Download")?;
    g.write(
        "/Windows/SoftwareDistribution/DataStore/DataStore.edb",
        b"Windows Update Database\n",
    )?;
    println!("  ✓ Windows Update metadata created");

    // Step 14: Create pagefile placeholder
    println!("\n[14/16] Creating system files...");
    g.write("/pagefile.sys", b"Windows Page File\n")?;
    g.write("/hiberfil.sys", b"Windows Hibernation File\n")?;
    g.write("/swapfile.sys", b"Windows Swap File\n")?;
    println!("  ✓ System files created");

    // Step 15: Test Phase 3 APIs
    println!("\n[15/16] Testing Phase 3 APIs on Windows image...");

    // Test stat()
    let stat = g.stat("/Windows/System32/version.txt")?;
    println!(
        "  ✓ stat(/Windows/System32/version.txt): size={} bytes",
        stat.size
    );

    // Test rm()
    g.write("/Temp/test-phase3.txt", b"test content")?;
    g.rm("/Temp/test-phase3.txt")?;
    println!("  ✓ rm() test passed");

    // Test rm_rf()
    g.mkdir_p("/Temp/test-dir/subdir")?;
    g.write("/Temp/test-dir/file1.txt", b"content1")?;
    g.write("/Temp/test-dir/subdir/file2.txt", b"content2")?;
    g.rm_rf("/Temp/test-dir")?;
    println!("  ✓ rm_rf() test passed");

    // Step 16: Create Windows Defender files
    println!("\n[16/16] Creating Windows Defender configuration...");
    g.mkdir_p("/ProgramData/Microsoft/Windows Defender")?;
    g.write(
        "/ProgramData/Microsoft/Windows Defender/platform.ini",
        b"[Windows Defender]\nVersion=4.18.24010.12\n",
    )?;
    println!("  ✓ Windows Defender configured");

    // Finalize
    println!("\n[Finalizing] Syncing and unmounting...");
    g.sync()?;
    g.umount_all()?;
    g.shutdown()?;

    println!(
        "\n=== {} Image Created Successfully! ===",
        version_meta.product_name
    );
    println!("  Image: {}", DISK_PATH);
    println!("  Size: {} MB", DISK_SIZE_MB);
    println!("  Filesystem: NTFS");
    println!("  Boot mode: {}", boot_mode);
    println!("  Edition: {}", version_meta.edition);
    println!("  Build: {}", version_meta.build);

    Ok(())
}

#[test]
fn test_windows_10_mbr() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n══════════════════════════════════════════════════════════════");
    println!("  Test: Windows 10 Pro - MBR/BIOS Layout");
    println!("══════════════════════════════════════════════════════════════");
    create_realistic_windows_image("10", false)
}

#[test]
fn test_windows_11_efi() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n══════════════════════════════════════════════════════════════");
    println!("  Test: Windows 11 Pro - EFI/GPT Layout");
    println!("══════════════════════════════════════════════════════════════");
    create_realistic_windows_image("11", true)
}

#[test]
fn test_windows_server_2022_efi() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n══════════════════════════════════════════════════════════════");
    println!("  Test: Windows Server 2022 - EFI/GPT Layout");
    println!("══════════════════════════════════════════════════════════════");
    create_realistic_windows_image("server2022", true)
}

#[test]
fn test_windows_inspection() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n══════════════════════════════════════════════════════════════");
    println!("  Test: Windows OS Inspection APIs");
    println!("══════════════════════════════════════════════════════════════\n");

    // First create a Windows image
    create_realistic_windows_image("11", true)?;

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
    assert_eq!(os_type, "windows", "Expected OS type 'windows'");
    println!("    ✓ OS Type: {}", os_type);

    // Test inspect_get_distro()
    println!("  Testing inspect_get_distro()...");
    let distro = g.inspect_get_distro(root)?;
    assert_eq!(distro, "windows", "Expected distro 'windows'");
    println!("    ✓ Distribution: {}", distro);

    // Test inspect_get_major_version()
    println!("  Testing inspect_get_major_version()...");
    let major = g.inspect_get_major_version(root)?;
    assert_eq!(major, 10, "Expected major version 10");
    println!("    ✓ Major Version: {}", major);

    g.shutdown()?;

    println!("\n=== All OS Inspection Tests Passed! ===\n");

    Ok(())
}

#[test]
fn test_windows_ntfs_features() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n══════════════════════════════════════════════════════════════");
    println!("  Test: Windows NTFS Feature Validation");
    println!("══════════════════════════════════════════════════════════════\n");

    // Create a Windows image
    create_realistic_windows_image("11", true)?;

    println!("\n[Testing NTFS Features]");

    // Open the image for testing
    let mut g = Guestfs::new()?;
    g.add_drive_ro(DISK_PATH)?;
    g.launch()?;

    // Mount and test
    g.mount("/dev/sda3", "/")?;

    // Test file listing
    println!("  Testing directory listing...");
    let files = g.ls("/Windows/System32")?;
    assert!(!files.is_empty(), "No files found in System32");
    println!("    ✓ Found {} files in System32", files.len());

    // Test file existence
    println!("  Testing file existence...");
    assert!(g.is_file("/Windows/System32/cmd.exe")?);
    println!("    ✓ cmd.exe exists");

    assert!(g.is_dir("/Program Files")?);
    println!("    ✓ Program Files is a directory");

    g.shutdown()?;

    println!("\n=== All NTFS Tests Passed! ===\n");

    Ok(())
}
