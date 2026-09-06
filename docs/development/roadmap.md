# Roadmap

High-level direction for GuestKit / GuestCtl. Full release notes are in [CHANGELOG.md](CHANGELOG.md).

## Shipped (recent)

- **Unreleased** — GuestKit-assured QEMU/VirtIO runtime (`guestkit-qemu plan|run|qmp`,
  `src/qemu/`) — evidence → boot gate → QEMU argv + QMP day-2 control; see
  [qemu-runtime.md](../features/qemu-runtime.md). Dump `virsh` from the live QGA
  path (`guestkit qga`, zyvor-api socket ladder) — [virsh-to-guestkit.md](../user-guides/virsh-to-guestkit.md)
- **v1.1.0** — Python assurance bindings (`run_doctor` / `run_migrate_repair`,
  PyPI `hypersdk-guestkit`), h2kvm offline fixer integration
- **v1.0.0** — GitHub Action for the Passport CI gate (`action.yml`,
  dogfooded by `passport-gate-demo.yml`); CI reliability pass (missing
  `libsystemd-dev`/`libhivex-dev` on Linux runners, loop/NBD device
  permissions, stale RPM spec version, doc-test compile fixes, k3s E2E
  musl/mingw/MinIO credential fixes, `zyvor-api` mTLS bootstrap-token
  crash-loop); Helm chart CI (`helm lint`/`template` across all real
  overlays) and a manual-dispatch workflow for the NBD tests hosted
  runners can't run; native OpenAI tool-calling (rig-core), cross-run AI
  memory, and an MCP server for the AI copilot (`guestkit mcp-serve`);
  fleet dependency-aware migration waves (`fleet wave-plan`) and
  scheduled drift monitoring against a stored baseline (`fleet watch`)
- **v0.3.21** — Fixed `rescue -o reset-password` SEGV (`hivex_value_type`
  FFI signature); `ntfsfix` now actually clears the NTFS dirty flag;
  fixed Windows cross-compile break from ungated Unix-only guestfs modules
- **v0.3.20** — DevOps runbooks + wiki; package mirror; Windows AES SAM / domain-leave RunOnce; UEFI fix-grub; host package fetch; worker profiles
- **v0.3.19** — Cutover Passport (`passport emit|verify`, BitLocker hard-block, live attestation, HyperSDK/hyper2kvm suite handoff); Windows day-0 `windows-domain-leave` / `windows-timezone` / `windows-static-ip`
- **v0.3.18** — Day-0 plans (`windows-hostname`, `windows-winrm`, hardened `linux-ssh` + key inject), `Symlink`/`FileDelete` ops, Windows SAM blank rescue, `rescue --export-plan`, offline `DriverInject` via `GUESTKIT_VIRTIO_WIN` / `migrate-repair --virtio-win`, heuristic offline remediations + preview live-only tags
- **v0.3.12** — Offline Windows registry writes in fix-plan apply (`registry-write` feature, libhivex FFI)
- **v0.3.11** — Guest Control Fabric (7-tier ladder, `guest/*` APIs, Agent Doctor, airgap poll)
- **v0.3.7–0.3.10** — `zyvor-guest-agent`, Windows forensics, KubeVirt QGA, web Copilot API, k3s E2E
- **v0.3.6** — In-guest agent, offline `--inject-agent`, worker jobs, TUI LIVE / Assurance
- **v0.3.5** — Migration assurance platform (`doctor`, `migrate-plan`, `policy`, `fleet`, `forensic-diff`)
- **v0.3.3–0.3.4** — `guestctl` binary, user release tarballs, TUI theming
- **v0.3.1** — VM migration (fstab/crypttab rewriter), Windows registry detection, LVM cleanup

### AI Guest Agent (all phases shipped)

Phases 0–4 of the optional AI layer are complete — richer systemd/Windows evidence, semantic analysis, the agentic loop (now with native rig-core tool-calling for OpenAI and cross-run memory), local Ollama + what-if simulation, and platform integration (CIS-lite profiles, Machina export, full `.evtx` forensics, an MCP server for external hosts). See [ai-guest-agent-roadmap.md](ai-guest-agent-roadmap.md).

## In progress / next

The previously tracked Unreleased slices (Passport signing, day-0/rescue, GCF telemetry, Helm, Windows depth, fleet parallel, cloud pulls, AES SAM, PackageInstall fetch, GRUB repair) are **shipped in Unreleased**. Next work is issue-driven — pick from [GitHub Issues](https://github.com/zyvorai/guestkit/issues) or propose a small PR.

## Parked (later)

None currently.

## Not planned (open source)

- Hosted control plane (see [zyvor-enterprise.md](../zyvor-enterprise.md))
- Automatic apply without dry-run/backup guardrails

## How to contribute

Pick an item from [GitHub Issues](https://github.com/zyvorai/guestkit/issues) or propose a small PR with tests. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Docs

- User guides: [docs/INDEX.md](../INDEX.md)
- Fix plans / rescue: [fix-plans.md](../features/fix-plans.md)
- QEMU runtime: [qemu-runtime.md](../features/qemu-runtime.md)
- Dump virsh: [virsh-to-guestkit.md](../user-guides/virsh-to-guestkit.md)
- CLI cheat sheet: [quick-reference.md](../user-guides/quick-reference.md)
- TUI: [tui-enhancements.md](../features/tui-enhancements.md)
