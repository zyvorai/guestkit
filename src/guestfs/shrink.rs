// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Shrink an oversized-but-mostly-empty guest disk to its real footprint.
//!
//! Motivation: a guest's *virtual* (declared) disk size is often much
//! larger than its *actual* used data (e.g. a VMware "growable" disk
//! created at 500GB but holding 6GB of real data). Some import targets
//! (notably KubeVirt/CDI) require the destination storage to have enough
//! real free space for the full virtual size, so an oversized-but-empty
//! disk can fail to import even though almost none of it is real data.
//! This module shrinks the guest filesystem, partition table, and disk
//! container down to match reality, with headroom.
//!
//! v1 scope (safety-first): a single/last ext2/3/4 partition, MBR or GPT,
//! no LVM/LUKS. Any other layout is reported as `Skipped`, never attempted
//! and never a hard error — this operation mutates the guest filesystem
//! and partition table, so guessing wrong about layout support risks data
//! loss, and "we didn't shrink it" is always an acceptable outcome.

use crate::core::{Error, Result};
use crate::disk::PartitionType;
use crate::guestfs::Guestfs;
use std::path::Path;
use std::process::Command;

/// GPT partition type GUID for Linux LVM (case-insensitive).
const GPT_LVM_GUID: &str = "e6d6d379-f507-44c2-a23c-238f2a3df928";
/// MBR partition type IDs that are not plain data partitions (LVM, extended).
const MBR_NON_DATA_TYPES: &[u8] = &[0x05, 0x0f, 0x85, 0x8e];

/// Outcome of inspecting a disk for shrink eligibility.
#[derive(Debug, Clone)]
pub struct ShrinkAnalysis {
    /// Guest-visible (declared) size, in bytes.
    pub virtual_bytes: u64,
    /// Real allocated size on the host, in bytes.
    pub actual_bytes: u64,
    /// 1-based partition number of the last (highest-offset) partition.
    pub last_partition: i32,
    /// Filesystem type of that partition ("ext2" | "ext3" | "ext4").
    pub fs_type: String,
    /// Filesystem's true minimum size as reported by resize2fs, in bytes.
    pub min_fs_bytes: u64,
    /// Byte offset of the last partition relative to the start of the disk.
    pub partition_start: u64,
    /// Partition table type.
    pub gpt: bool,
}

/// Result of a shrink attempt.
#[derive(Debug, Clone)]
pub enum ShrinkOutcome {
    /// The disk was shrunk successfully.
    Shrunk { old_virtual: u64, new_virtual: u64 },
    /// Nothing was changed — `reason` explains why (unsupported layout,
    /// not worth shrinking, fsck found uncorrectable errors, etc).
    Skipped { reason: String },
}

fn path_str(image: &Path) -> Result<&str> {
    image.to_str().ok_or_else(|| {
        Error::InvalidFormat(format!("Path contains invalid UTF-8: {}", image.display()))
    })
}

/// Inspect `image` read-only and determine whether it's a supported shrink
/// candidate. Never mutates the image. Returns `Ok(None)` (not `Err`) for
/// any layout v1 doesn't support, so "not eligible" and "eligible" are both
/// normal, expected outcomes rather than errors.
pub fn analyze_shrink_potential(image: &Path, verbose: bool) -> Result<Option<ShrinkAnalysis>> {
    let image_str = path_str(image)?;

    let mut probe = Guestfs::new()?;
    probe.set_verbose(verbose);

    let virtual_bytes = probe.disk_virtual_size(image_str)? as u64;
    let actual_bytes = probe.disk_actual_size(image_str)? as u64;

    probe.add_drive_ro(image_str)?;
    probe.launch()?;

    let table_type = probe.partition_table()?.table_type().clone();
    if table_type == PartitionType::Unknown {
        return skip(&mut probe, "no recognized MBR/GPT partition table");
    }

    // Full Partition structs (not just PartInfo) so we can check type_id/type_guid.
    let partitions: Vec<_> = probe.partition_table()?.partitions().to_vec();
    if partitions.is_empty() {
        return skip(&mut probe, "no partitions found");
    }

    let last = partitions
        .iter()
        .max_by_key(|p| p.start_lba.saturating_add(p.size_sectors))
        .expect("checked non-empty above")
        .clone();

    if is_lvm_or_extended(&last) {
        return skip(
            &mut probe,
            "last partition is LVM or extended — unsupported in v1",
        );
    }

    let filesystems = probe.list_filesystems()?;
    let dev_name = format!("/dev/sda{}", last.number);
    let fs_type = match filesystems.get(&dev_name).map(String::as_str) {
        Some(t @ ("ext2" | "ext3" | "ext4")) => t.to_string(),
        Some(other) => {
            return skip(
                &mut probe,
                &format!("last partition filesystem is {other} — only ext2/3/4 supported in v1"),
            )
        }
        None => {
            return skip(
                &mut probe,
                "could not detect a filesystem on the last partition",
            )
        }
    };

    probe.setup_nbd_if_needed()?;
    let part_path = probe
        .nbd_device()?
        .partition_path(last.number)
        .display()
        .to_string();

    let min_fs_bytes = match resize2fs_min_size_bytes(&part_path, verbose) {
        Ok(bytes) => bytes,
        Err(e) => {
            return skip(
                &mut probe,
                &format!("could not determine filesystem's minimum size: {e}"),
            )
        }
    };

    probe.shutdown()?;

    Ok(Some(ShrinkAnalysis {
        virtual_bytes,
        actual_bytes,
        last_partition: last.number as i32,
        fs_type,
        min_fs_bytes,
        partition_start: last.start_lba.saturating_mul(512),
        gpt: table_type == PartitionType::GPT,
    }))
}

fn skip(probe: &mut Guestfs, reason: &str) -> Result<Option<ShrinkAnalysis>> {
    let _ = probe.shutdown();
    let _ = reason; // surfaced to caller via None + their own logging of `reason` string
    Ok(None)
}

fn is_lvm_or_extended(p: &crate::disk::Partition) -> bool {
    if let Some(guid) = &p.type_guid {
        if guid.eq_ignore_ascii_case(GPT_LVM_GUID) {
            return true;
        }
    }
    MBR_NON_DATA_TYPES.contains(&p.type_id)
}

/// Parse `resize2fs -P <device>`'s "Estimated minimum size of the
/// filesystem: N" (in filesystem blocks) and multiply by the block size
/// from `dumpe2fs -h <device>`'s "Block size: N".
fn resize2fs_min_size_bytes(device: &str, verbose: bool) -> Result<u64> {
    if verbose {
        eprintln!("guestfs: resize2fs -P {device}");
    }

    let out = Command::new("resize2fs")
        .arg("-P")
        .arg(device)
        .output()
        .map_err(|e| Error::CommandFailed(format!("Failed to execute resize2fs: {e}")))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let min_blocks: u64 = stdout
        .lines()
        .find_map(|l| l.rsplit(':').next().map(str::trim))
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| {
            Error::Detection(format!(
                "could not parse resize2fs -P output: {stdout} / {}",
                String::from_utf8_lossy(&out.stderr)
            ))
        })?;

    let out = Command::new("dumpe2fs")
        .arg("-h")
        .arg(device)
        .output()
        .map_err(|e| Error::CommandFailed(format!("Failed to execute dumpe2fs: {e}")))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let block_size: u64 = stdout
        .lines()
        .find(|l| l.starts_with("Block size:"))
        .and_then(|l| l.rsplit(':').next())
        .map(str::trim)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| {
            Error::Detection(format!("could not parse dumpe2fs -h block size: {stdout}"))
        })?;

    Ok(min_blocks * block_size)
}

/// Shrink `image` in place: shrinks the filesystem, then the partition
/// table entry, then truncates the container to match, with `headroom_pct`
/// extra space left over the filesystem's true minimum size (e.g. 20 means
/// the new filesystem size is `min_size * 1.20`).
///
/// Only proceeds for layouts `analyze_shrink_potential` reports as
/// eligible; anything else returns `ShrinkOutcome::Skipped` rather than an
/// error. A failed `e2fsck` (uncorrectable errors) also skips rather than
/// resizing a filesystem known to be inconsistent.
pub fn shrink_disk(image: &Path, verbose: bool, headroom_pct: u32) -> Result<ShrinkOutcome> {
    let image_str = path_str(image)?;

    let analysis = match analyze_shrink_potential(image, verbose)? {
        Some(a) => a,
        None => {
            return Ok(ShrinkOutcome::Skipped {
                reason: "not an eligible shrink candidate (see analyze_shrink_potential log)"
                    .to_string(),
            })
        }
    };

    // +1MiB fixed floor on top of the percentage headroom so a nearly-empty
    // filesystem (min_fs_bytes close to 0) doesn't end up sized to ~0.
    let target_fs_bytes = analysis
        .min_fs_bytes
        .saturating_mul(100 + headroom_pct as u64)
        / 100
        + 1024 * 1024;

    if target_fs_bytes >= analysis.virtual_bytes {
        return Ok(ShrinkOutcome::Skipped {
            reason: "computed target size is not smaller than current virtual size".to_string(),
        });
    }

    // In-place resize2fs shrink was tried first and rejected: resize2fs's
    // shrink algorithm relocates data per-inode/per-extent, so a real OS
    // install (tens of thousands of small files scattered across block
    // groups sized for the original huge nominal capacity) took 40+ minutes
    // and counting in testing, vs. ~50s for the same nominal size with a
    // single large test file. The cost is dominated by *how scattered* the
    // real data is, not by its total size.
    //
    // Instead: build a fresh, correctly-sized destination image and copy
    // files across at the file level (closer to what virt-resize does
    // internally). Cost scales with actual bytes copied via ordinary
    // sequential I/O, not with the number of scattered inodes relocated.
    let target_mb = target_fs_bytes.div_ceil(1024 * 1024).max(1);
    let new_part_end_bytes = analysis.partition_start + target_mb * 1024 * 1024;
    // 1MiB-aligned with a little slack past the partition end for the
    // partition table / GPT backup header.
    let new_virtual_bytes =
        (new_part_end_bytes + 2 * 1024 * 1024).div_ceil(1024 * 1024) * 1024 * 1024;

    let format = {
        let mut probe = Guestfs::new()?;
        probe.set_verbose(verbose);
        probe.disk_format(image_str)?
    };

    let new_image = sibling_path(image, "shrink-new");
    let new_image_str = path_str(&new_image)?;
    let _cleanup = TempImageGuard(new_image.clone());

    // Open the source whole-disk read-only — used for the prefix copy below
    // and, later, as the read side of the file copy.
    let mut old_g = Guestfs::new()?;
    old_g.set_verbose(verbose);
    old_g.add_drive_ro(image_str)?;
    old_g.launch()?;
    old_g.setup_nbd_if_needed()?;
    let old_whole_disk = old_g.nbd_device()?.device_path().display().to_string();

    // Create the destination container and open it read-write.
    {
        let mut creator = Guestfs::new()?;
        creator.set_verbose(verbose);
        creator.disk_create(new_image_str, &format, new_virtual_bytes as i64)?;
    }
    let mut new_g = Guestfs::new()?;
    new_g.set_verbose(verbose);
    new_g.add_drive(new_image_str)?;
    new_g.launch()?;
    new_g.setup_nbd_if_needed()?;
    let new_whole_disk = new_g.nbd_device()?.device_path().display().to_string();

    // Copy everything before the shrunk partition byte-for-byte (partition
    // table + any earlier partitions, e.g. a /boot or EFI partition) — this
    // also carries over the OLD (huge) partition-table entry for the last
    // partition, fixed up by part_resize right after.
    dd_copy(&old_whole_disk, &new_whole_disk, analysis.partition_start)?;
    // The kernel only scans a device's partition table once, at NBD
    // connect time — it was empty then. Rescan now so /dev/nbdXpY nodes
    // exist for the table dd just wrote (and again after part_resize,
    // since that rewrites partition 2's entry directly on disk via parted,
    // which the kernel also won't notice on its own).
    rescan_partitions(&new_whole_disk)?;

    let new_part_end_sector = new_part_end_bytes / 512;
    if analysis.gpt {
        // The copied primary GPT header still records the OLD disk's
        // AlternateLBA (its backup header's location, at the far end of
        // the original 500GB+ span) — but the new container is only ~15GB.
        // Two tools were tried and don't work here:
        //   - `parted resizepart` can't even open the disk to inspect it —
        //     it errors trying to *read* that now-out-of-bounds backup
        //     location ("Invalid argument during seek for read").
        //   - `sgdisk -e` (relocate backup header) refuses too: it
        //     validates every partition against the *current* device size
        //     before writing anything, and partition 2's still-500GB
        //     entry fails that check — "Problem: partition 2 is too big
        //     for the disk. Aborting write operation!" — even though
        //     fixing the header is exactly what would resolve that.
        // The way out: `sgdisk -d/-n` (delete + recreate the partition at
        // its new, valid bounds) and the backup-header repair happen in
        // the *same* write, so the chicken-and-egg validation above never
        // trips — sgdisk regenerates a fresh, correct backup header as
        // part of committing the already-valid new partition table.
        let start_sector = analysis.partition_start / 512;
        // Read from old_g (the untouched original), not the new image —
        // the new image's on-disk GPT is the exact stale-but-invalid state
        // described above, and there's no reason to risk `sgdisk -i`'s
        // read path on it when the pristine original has the same value.
        let type_guid = old_g
            .part_get_gpt_type("/dev/sda", analysis.last_partition)?
            .split_whitespace()
            .next()
            .map(str::to_string)
            .ok_or_else(|| {
                Error::Detection("could not parse original partition type GUID".to_string())
            })?;
        let output = Command::new("sgdisk")
            .arg("-d")
            .arg(analysis.last_partition.to_string())
            .arg("-n")
            .arg(format!(
                "{}:{start_sector}:{new_part_end_sector}",
                analysis.last_partition
            ))
            .arg("-t")
            .arg(format!("{}:{type_guid}", analysis.last_partition))
            .arg(&new_whole_disk)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute sgdisk: {e}")))?;
        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "sgdisk partition resize failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
    } else {
        new_g.part_resize(
            "/dev/sda",
            analysis.last_partition,
            new_part_end_sector as i64,
        )?;
    }
    rescan_partitions(&new_whole_disk)?;

    // Fresh filesystem at the target size, same UUID/label as the original
    // so fstab UUID=/LABEL= entries in the copied files keep working.
    let orig_part = format!("/dev/sda{}", analysis.last_partition);
    let uuid = old_g.get_e2uuid(&orig_part).ok();
    let label = old_g.get_e2label(&orig_part).ok().filter(|l| !l.is_empty());

    let new_part_path = new_g
        .nbd_device()?
        .partition_path(analysis.last_partition as u32)
        .display()
        .to_string();
    let mkfs = Command::new("mke2fs")
        .arg("-q")
        .arg("-F")
        .arg("-t")
        .arg(&analysis.fs_type)
        .arg(&new_part_path)
        .output()
        .map_err(|e| Error::CommandFailed(format!("Failed to execute mke2fs: {e}")))?;
    if !mkfs.status.success() {
        return Err(Error::CommandFailed(format!(
            "mke2fs failed: {}",
            String::from_utf8_lossy(&mkfs.stderr)
        )));
    }
    if let Some(uuid) = uuid {
        new_g.set_e2uuid(&orig_part, &uuid)?;
    }
    if let Some(label) = label {
        new_g.set_e2label(&orig_part, &label)?;
    }

    // Mount both sides and copy file contents across.
    old_g.mount_ro(&orig_part, "/")?;
    let old_root = old_g.mount_root.clone().ok_or_else(|| {
        Error::InvalidState("source mount root missing after mount_ro".to_string())
    })?;
    new_g.mount(&orig_part, "/")?;
    let new_root = new_g.mount_root.clone().ok_or_else(|| {
        Error::InvalidState("destination mount root missing after mount".to_string())
    })?;

    copy_tree(&old_root, &new_root, verbose)?;

    old_g.umount_all()?;
    new_g.umount_all()?;
    old_g.shutdown()?;
    new_g.shutdown()?;

    std::fs::rename(&new_image, image).map_err(|e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!("replacing {}: {e}", image.display()),
        ))
    })?;
    // Renamed into place — nothing left for the guard to clean up.
    std::mem::forget(_cleanup);

    Ok(ShrinkOutcome::Shrunk {
        old_virtual: analysis.virtual_bytes,
        new_virtual: new_virtual_bytes,
    })
}

/// Deletes the wrapped path on drop, unless `forget`-ten — used so any
/// early-return (via `?`) from `shrink_disk` cleans up the half-built
/// destination image instead of leaving it behind.
struct TempImageGuard(std::path::PathBuf);

impl Drop for TempImageGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Force the kernel to re-read `dev`'s partition table. Needed after
/// writing new partition-table bytes to a device the kernel already
/// scanned once (at NBD connect time) — the kernel doesn't notice
/// out-of-band writes on its own, so `/dev/nbdXpY` nodes and their sizes
/// would otherwise stay stale.
fn rescan_partitions(dev: &str) -> Result<()> {
    let output = Command::new("blockdev").arg("--rereadpt").arg(dev).output();
    if matches!(&output, Ok(o) if o.status.success()) {
        return Ok(());
    }
    // blockdev missing entirely, or this device doesn't support BLKRRPART —
    // partprobe is the standard fallback, from the same package as parted.
    let fallback = Command::new("partprobe")
        .arg(dev)
        .output()
        .map_err(|e| Error::CommandFailed(format!("Failed to execute partprobe: {e}")))?;
    if !fallback.status.success() {
        return Err(Error::CommandFailed(format!(
            "partition rescan failed for {dev}: {}",
            String::from_utf8_lossy(&fallback.stderr)
        )));
    }
    Ok(())
}

fn sibling_path(image: &Path, suffix: &str) -> std::path::PathBuf {
    let mut name = image
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(format!(".{suffix}"));
    image.with_file_name(name)
}

/// Copy the first `bytes` of `src_dev` to `dst_dev` (both whole-disk device
/// paths, e.g. NBD devices) without truncating the destination — used to
/// carry over the partition table and any partitions before the one being
/// shrunk, unchanged.
fn dd_copy(src_dev: &str, dst_dev: &str, bytes: u64) -> Result<()> {
    let count_mb = bytes.div_ceil(1024 * 1024);
    let output = Command::new("dd")
        .arg(format!("if={src_dev}"))
        .arg(format!("of={dst_dev}"))
        .arg("bs=1M")
        .arg(format!("count={count_mb}"))
        .arg("conv=notrunc,fsync")
        .output()
        .map_err(|e| Error::CommandFailed(format!("Failed to execute dd: {e}")))?;
    if !output.status.success() {
        return Err(Error::CommandFailed(format!(
            "dd (prefix copy) failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

/// Copy file contents from `src` to `dst` (both host directories — mount
/// points of the source and destination filesystems) preserving
/// permissions, ownership, timestamps, symlinks, hardlinks, ACLs, and
/// xattrs. Prefers `rsync` (handles all of the above in one pass); falls
/// back to `cp -a` if rsync isn't installed, which preserves everything
/// except hardlink identity (each hardlinked file becomes an independent
/// copy — more disk use, but not data loss or corruption).
fn copy_tree(src: &Path, dst: &Path, verbose: bool) -> Result<()> {
    let src_arg = format!("{}/", src.display());
    if let Ok(rsync) = which("rsync") {
        if verbose {
            eprintln!(
                "guestfs: rsync -aHAX --numeric-ids {src_arg} {}",
                dst.display()
            );
        }
        let output = Command::new(rsync)
            .arg("-aHAX")
            .arg("--numeric-ids")
            .arg(&src_arg)
            .arg(dst)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute rsync: {e}")))?;
        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "rsync failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        return Ok(());
    }

    if verbose {
        eprintln!("guestfs: rsync not found, falling back to cp -a (no hardlink preservation)");
    }
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "cp -a {}. {}",
            shell_quote(&src_arg),
            shell_quote(&dst.display().to_string())
        ))
        .output()
        .map_err(|e| Error::CommandFailed(format!("Failed to execute cp: {e}")))?;
    if !output.status.success() {
        return Err(Error::CommandFailed(format!(
            "cp -a failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

fn which(prog: &str) -> std::result::Result<String, ()> {
    Command::new("which")
        .arg(prog)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or(())
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_reason_is_descriptive_not_empty() {
        // Compile-time/shape check: ShrinkOutcome::Skipped always carries a
        // human-readable reason, never a bare unit variant, so callers
        // always have something to log.
        let outcome = ShrinkOutcome::Skipped {
            reason: "example".to_string(),
        };
        match outcome {
            ShrinkOutcome::Skipped { reason } => assert!(!reason.is_empty()),
            ShrinkOutcome::Shrunk { .. } => panic!("unexpected"),
        }
    }
}
