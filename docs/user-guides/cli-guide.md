# CLI reference (v1.2.4)

`guestkit` and `guestctl` share the same command surface. The separate
**`guestkit-qemu`** binary plans and launches QEMU from the same assurance
engine. Use **`guestkit --help`**, **`guestkit <command> --help`**, and
**`guestkit-qemu --help`** for flags; this page links the curated docs.

## Quick start

```bash
guestkit inspect disk.qcow2
guestkit doctor disk.qcow2 --target proxmox --explain
guestkit doctor disk.qcow2 --target proxmox -o json --fail-below 80
guestkit migrate-plan disk.qcow2 --target kvm --export plan.yaml
guestctl tui disk.qcow2
guestkit-qemu plan disk.qcow2 --json
guestkit-qemu run disk.qcow2 --min-boot-score 80
guestkit qga --execute guest-ping
```

## Where to look

| Topic | Doc |
|-------|-----|
| Cheat sheet | [quick-reference.md](quick-reference.md) |
| Install & build | [getting-started.md](getting-started.md) |
| Migration assurance | [migration-assurance.md](../features/migration-assurance.md) |
| QEMU / VirtIO runtime | [qemu-runtime.md](../features/qemu-runtime.md) |
| Dump virsh → GuestKit | [virsh-to-guestkit.md](virsh-to-guestkit.md) |
| Guest agent | [guest-agent.md](../features/guest-agent.md) |
| VM migration workflows | [vm-migration.md](vm-migration.md) |
| TUI keys & Assurance | [tui-enhancements.md](../features/tui-enhancements.md) |
| Fix plans & rescue | [fix-plans.md](../features/fix-plans.md) |
| Profiles | [profiles.md](profiles.md) |
| Interactive REPL | [interactive-mode.md](interactive-mode.md) |
| File explorer | [EXPLORE-QUICKSTART.md](../features/explore/EXPLORE-QUICKSTART.md) |
| Cloud disk sources | [cloud-disk-sources.md](../guides/cloud-disk-sources.md) |
| Python API | [python-bindings.md](python-bindings.md) |
| Serial console without grubby | [img-firstboot.md](img-firstboot.md#serial-console-without-grubby) |
| FAQ | [faq.md](faq.md) |
| Troubleshooting | [troubleshooting.md](troubleshooting.md) |

## Command groups

| Group | Examples |
|-------|----------|
| Inspect | `inspect`, `filesystems`, `packages`, `services`, `users`, `network` |
| Files | `ls`, `cat`, `cp`, `download`, `upload`, `find` |
| Assurance | `doctor`, `migrate-plan`, `policy`, `fleet`, `forensic-diff`, `repair --fix boot`, `passport emit\|verify` |
| QEMU runtime | `guestkit-qemu plan`, `guestkit-qemu run`, `guestkit-qemu qmp` |
| Live QGA / agent | `qga`, `agent-call`, `agent-proxy`, `agent-inject` |
| Plans | `plan generate`, `plan preview`, `plan apply` (`--skip-backup`), `plan rollback` |
| Rescue | `rescue -o enable-ssh\|inject-ssh-key\|set-hostname\|reset-password\|fix-fstab\|check-grub\|fix-grub\|enable-rdp\|enable-winrm\|set-timezone` |
| Profiles | `profile security`, `profile windows-migration` |
| TUI | `guestctl tui`, `guestkit tui` |
| Shell | `guestkit shell`, `guestkit interactive` |

List all commands: **`guestkit commands`** (or **`guestkit command-catalog`**).

## Useful environment variables

| Variable | Purpose |
|----------|---------|
| `GUESTKIT_PACKAGE_CACHE` | Host dirs of `.rpm`/`.deb` for offline PackageInstall staging |
| `GUESTKIT_PACKAGE_FETCH` | `1`/`true` — host-download missing packages before staging |
| `GUESTKIT_PACKAGE_MIRROR` | HTTP base URL(s) for package fetch fallback (`curl`/`wget`) |
| `GUESTKIT_VIRTIO_WIN` | VirtIO driver tree for offline Windows DriverInject |
| `GUESTKIT_FLEET_JOBS` | Parallelism for `fleet analyze` (default min(4, CPUs)) |
| `GUESTKIT_S3_ENDPOINT` / `AWS_ENDPOINT_URL` | Custom S3-compatible endpoint for cloud disk pulls |

Details: [fix-plans.md](../features/fix-plans.md), [cloud-disk-sources.md](../guides/cloud-disk-sources.md).

## Disk formats

QCOW2, VMDK, VDI, VHD, RAW, IMG — auto-detected. RAW/IMG often use loop devices; QCOW2/VMDK use NBD. Use **`--trace`** to see which path is used.

## JSON output

Most inspect commands accept **`-o json`** or **`--json`** for scripting. See [quick-reference.md](quick-reference.md) for examples.

## See also

- [Documentation index](../INDEX.md)
- [Changelog](../development/CHANGELOG.md)
- [Roadmap](../development/roadmap.md)
- [zyvor.dev/guestkit](https://zyvor.dev/guestkit)
