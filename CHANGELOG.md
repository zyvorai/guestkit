# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.2.2] - 2026-09-06

### Added
- **Web Image Vault TUI parity** — OSS `deploy/ui` detail workspace: inspect inventory tabs, Assurance (plan/passport/repair preview + gated apply), Profiles, and Files browse.
- **API / worker** — `POST /api/v1/vms/:id/profile`, `POST /api/v1/vms/:id/explore` (`guestkit.explore` ls/stat/cat), repair `dry_run` query; larger inspect samples.

## [1.2.1] - 2026-09-03

### Security
- Patched RUSTSEC-2026-0176/0177 (pyo3 0.27.2 -> 0.29.2: out-of-bounds read in `PyList`/`PyTuple` iterators, missing `Sync` bound on `PyCFunction::new_closure` closures) and RUSTSEC-2026-0258 (h2 0.4.14 -> 0.4.16: unbounded empty DATA frames). `cargo audit` now reports zero vulnerabilities.
- Verified the pyo3 bump beyond `cargo check`: a real `maturin build --release --features python-bindings` produced a working wheel, installed cleanly into a fresh venv, and `import guestkit` reports the right version -- no breaking API changes hit this crate's bindings code between 0.27 and 0.29.

## [1.2.0] - 2026-09-03

### Added
- **GuestKit-assured QEMU/VirtIO runtime and `guestkit-qemu` CLI**, replacing `virsh qemu-agent-command` with direct QGA socket access.
- **`guestkit vm`** — local QEMU lifecycle management without libvirt.
- **`virtctl-guestkit guestfs`** — a drop-in for `virtctl guestfs`.
- **Cutover bundle**: gate, SELinux/sysprep/BitLocker handling, virtio-initramfs, agent-sign.
- **Cloud cutover profiles**, Rego policy checks, and an offline cloud-init datasource.
- **Passport handoff, fleet quarantine**, and the `virtctl-guestkit` plugin.
- **`guestkit img`, domain-disks, virtio-win, and firstboot** commands.
- `sbom-diff`, rescue dry-run Action, and Passport Action extras.
- **`guestkit shrink <image>`** -- shrinks an oversized-but-mostly-empty guest disk's declared virtual size to match its real data footprint, so it stops looking far bigger than it actually is to downstream tools. Motivated by a real KubeVirt/CDI import failure: a VMware "growable" VMDK declared at 500GB virtual holding 6GB of actual data uploaded successfully, then was rejected by CDI at the finish line because the cluster's storage didn't have 500GB free -- CDI requires the destination to fit the full declared virtual size regardless of real usage.
  - `--dry-run` -- report what would happen without changing anything.
  - `--min-ratio N` (default `3.0`) -- only shrink when virtual/actual size ratio is at least this much.
  - `--headroom-pct N` (default `20`) -- extra space left over the filesystem's true minimum size.
  - `--json` -- machine-readable output, consumed by hyper2kvm's `--shrink-oversized-disk` pipeline step.
  - v1 scope is intentionally narrow for safety: a single/last ext2/3/4 partition, MBR or GPT, no LVM/LUKS -- any other layout is reported and left untouched, never a hard error and never a guess, since this operation mutates the guest filesystem and partition table.
  - **Implementation: build a fresh, correctly-sized destination and copy files across (`rsync -aHAX`), not an in-place `resize2fs` shrink.** In-place shrink was tried first and rejected by measurement: `resize2fs` relocates data per-inode, so a real OS install (tens of thousands of files scattered across block groups sized for the original huge nominal capacity) took 40+ minutes and was still running, vs. ~50s for the same nominal size with one large test file -- the cost is dominated by how scattered the real data is, not its total size. The copy-based approach's cost scales with actual bytes copied (a couple of minutes for ~6GB), matching what `virt-resize` does internally. No new dependency on the libguestfs appliance -- built from the same lightweight tools (`resize2fs`, `sgdisk`, `mke2fs`, `rsync`, `dd`, `qemu-img`) guestkit already shells to elsewhere.
  - GPT disks are resized via `sgdisk -d/-n` (delete + recreate the partition, new bounds and backup-header repair in one write) rather than `parted resizepart` + `sgdisk -e`: after copying the original partition table bytes into the smaller destination container, the copied primary GPT header still points its backup/AlternateLBA at the *original* disk's end -- `parted` can't even open the disk to inspect it ("Invalid argument during seek for read"), and `sgdisk -e` alone refuses too ("Problem: partition 2 is too big for the disk. Aborting write operation!", since it validates every partition against the current device size before writing anything, and the still-huge partition 2 fails that check even though fixing the header is what would resolve it). Deleting and recreating the partition at its new, valid bounds sidesteps that chicken-and-egg validation entirely.
  - New `Guestfs::disk_actual_size()` and `Guestfs::disk_resize_shrink()` in `guestfs::disk_mgmt`.

### Fixed
- **`disk_virtual_size()` / `disk_format()` returned the wrong value for VMDK and other formats with a nested `children` block in `qemu-img info --output=json`** -- both parsed the JSON by scanning raw text for the first line containing `"virtual-size"`/`"format"`, but qemu-img's JSON nests a `children[].info` object (the underlying file's own size/format) that repeats those same keys *earlier* in the text than the real top-level values. For a VMDK this silently returned the file's raw byte length (e.g. 6GB) instead of its declared virtual size (e.g. 500GB) -- caught while building the `shrink` command above, whose whole purpose is telling those two numbers apart. Both now parse the full JSON via `serde_json` and read only the top-level keys.
- **Two simultaneous `Guestfs` handles in one process silently mounted on top of each other, discarding one filesystem's contents** (`guestfs::mount::create_mount_root`) -- the per-handle mount directory was named `guestkit-<pid>` using only the process ID, so a second handle created in the same process (needed by `shrink` for its simultaneous source + destination filesystems) computed the *identical* path; `fs::create_dir_all` on an already-existing directory succeeds silently, so the second `mount()` call mounted its filesystem directly on top of the first's, shadowing it. Caught by verifying `shrink`'s output disk's actual contents rather than trusting its "success" log line -- the very first working end-to-end run had silently produced an empty destination filesystem (`lost+found` only, all real data discarded). Fixed with a per-process atomic counter appended to the directory name; every prior caller only ever used one handle at a time, so this was previously latent.
- `guestfs::mount()` now remounts read-write when a caller's mount hits an already-mounted device, instead of failing.
- Gate passport writes on non-writable image directories; companion CLIs are now installed on deploy.
- BOOT-003 false positive when `/boot` is a separate fstab partition.
- `guestkit qga_client` build failure (missing `FileTypeExt` import for `is_socket`).
- Stopped flagging `open-vm-tools` as a proprietary VMware remnant.
- Patched RUSTSEC-2026-0185 (quinn-proto, remote memory exhaustion via unbounded out-of-order stream reassembly) and cleared rustfmt drift that was blocking CI.
- Various stale CLI names, dead links, and a license contradiction in the docs.

### Security
- RUSTSEC-2026-0185 (see above).

### Docs
- Documented the split: GuestKit certifies disks, Ephemera (now FluxVM) runs and manages VMs — including pointing the suite handoff and virsh guide at FluxVM.

