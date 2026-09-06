# GuestKit — Feature Guide

> **Offline VM intelligence and migration assurance.**

GuestKit is a pure-Rust control plane that reads a virtual machine's disk image offline, builds a normalized evidence snapshot, and answers the two questions that decide every migration: will it boot, and what must change before cutover. It ships as a scriptable CLI (guestkit), a carbon-themed TUI (guestctl), an assured QEMU launcher (guestkit-qemu), Python bindings, and a self-hosted web platform with KubeVirt integration - all sharing one engine, with no appliance daemon to install.

**70+** CLI subcommands · **6** disk formats read · ****0** appliance daemons needed · **8** migration targets scored · **0-100** boot assurance score · **5** inspection profiles

This is the user-facing onboarding guide — how to access the product, your first workflows, and how to use every feature. A print-ready PDF of the same content sits alongside this file.

## Contents

0. [Getting started — access & first workflows](#getting-started)
1. [Offline Disk Inspection](#1-offline-disk-inspection)
2. [Migration Assurance](#2-migration-assurance)
3. [Fix Plans & Repair](#3-fix-plans-repair)
4. [Security & Threat Analysis](#4-security-threat-analysis)
5. [Inventory & Reporting](#5-inventory-reporting)
6. [Planning & Optimization](#6-planning-optimization)
7. [Interactive Workspaces](#7-interactive-workspaces)
8. [Guest Agent & Live Control](#8-guest-agent-live-control)
9. [KubeVirt & Platform](#9-kubevirt-platform)
10. [QEMU / VirtIO runtime](#10-qemu-virtio-runtime)
11. [Deployment & Editions](#11-deployment-editions)

## Getting started

**How to access it**

- **Web:** Self-hosted web console at http://localhost:8088 (nginx front-end proxies `/api/` to the zyvor-api backend). Start it with `docker compose -f deploy/docker-compose.ghcr.yml up -d`.
- **CLI:** Three host binaries: `guestkit` (scriptable CLI), `guestctl` (TUI), and `guestkit-qemu` (assured QEMU plan/run/QMP). Install with `cargo install guestkit`; run `guestkit --help` or `guestkit commands` to list subcommands. Launch the TUI with `guestctl tui vm.qcow2`.
- **API:** REST API served by zyvor-api behind the console's `/api/` path (e.g. `http://localhost:8088/api/`); it enqueues inspect/boot-inspect jobs onto the Redis-backed worker. Live guests are reachable host-side via `guestkit agent-proxy --listen 127.0.0.1:8765` (e.g. `curl http://127.0.0.1:8765/doctor`). Python bindings expose the engine in-process via `from guestkit import Guestfs`.
- **Login:** Packaged/web installs seed a default administrator: username `admin`, password `Admin@321` (also the default API key where applicable). Change the password, API key and `JWT_SECRET` immediately after first login and enable SSO/SAML from Settings before any network exposure.
- **Needs:** A Linux host with qemu-img, losetup and qemu-nbd installed; mount and in-cluster boot-inspect need root or a privileged pod. `guestkit-qemu run` also needs a QEMU system binary on `PATH`.

**Your first workflows**

- **Pre-flight environment check**
  1. Install the tools: `cargo install guestkit` (installs `guestkit`, `guestctl`, and `guestkit-qemu`).
  1. Verify host tooling is present: `guestkit doctor --help` and confirm qemu-img/losetup/qemu-nbd are installed.
  1. Confirm the disk opens and its format is detected: `guestkit detect vm.qcow2`.
- **Assurance-first migration (recommended)**
  1. Score boot readiness with root-cause: `guestkit doctor vm.vmdk --target proxmox --explain`.
  1. Generate the hypervisor-aware checklist: `guestkit migrate-plan vm.vmdk --target proxmox -o json > migration-plan.json`.
  1. Capture Windows-specific signals if needed: `guestkit inspect vm.vmdk --profile windows-migration -o json`.
  1. Fix boot blockers offline and re-score: `guestkit repair vm.vmdk --fix boot --dry-run`, then `guestkit repair vm.vmdk --fix boot`, then `guestkit doctor vm.vmdk --target proxmox`.
- **Convert and cut over to KVM**
  1. Transcode the disk if required: `guestkit convert vm.vmdk --output vm.qcow2 --format qcow2 --compress`.
  1. Inspect the converted image: `guestkit inspect vm.qcow2 --profile migration -o json > vm-inventory.json`.
  1. Export a reviewable fix plan: `guestkit migrate-plan vm.qcow2 --target kvm --export plan.yaml`.
  1. Apply with backups and re-verify: `guestkit plan apply plan.yaml` (roll back with `guestkit plan rollback` if needed).
- **Gate a golden-image pipeline**
  1. Run doctor in JSON with a threshold: `guestkit doctor img.qcow2 --target proxmox -o json --fail-below 80`.
  1. Enforce sign-off rules as code: `guestkit policy check img.qcow2` (DSL over evidence fields or a CIS benchmark).
  1. Fail the CI job on any non-zero exit so regressed images never ship.
- **Interactive investigation in the TUI**
  1. Open the dashboard: `guestctl tui vm.qcow2`.
  1. Jump to Assurance with `a`, then run doctor `d`, cycle target `t`, preview the fix plan `p`, export YAML `e`.
  1. Spelunk files with `guestkit explore vm.qcow2`, or compare two VMs via `guestctl tui vm.qcow2 --compare other.qcow2`.

## 1. Offline Disk Inspection

_Read the full guest OS from a cold disk image - no boot, no agent, no appliance._

- **One-command inspect** — guestkit inspect surfaces OS, distro, version, hostname, architecture, and init system from any supported disk image in a single pass. — _Know exactly what a VM is before you touch it._
  - **How:** CLI: `guestkit inspect disk.qcow2` (add `-o json` for automation). In the TUI, the Summary view shows the same fields.
- **Six disk formats, auto-detected** — Reads QCOW2, VMDK, VHD, VHDX, VDI, and RAW/IMG images, choosing loop devices or qemu-nbd automatically. — _Point it at whatever your hypervisor exported - it just opens._
  - **How:** CLI: `guestkit detect disk.img` confirms the format; any command (`inspect`, `doctor`, `explore`) opens QCOW2/VMDK/VHD/VHDX/VDI/RAW directly. Add `--trace` to see the loop vs qemu-nbd path chosen.
- **Guest OS detection** — Fingerprints Linux distributions (Fedora, Ubuntu, Debian, RHEL, CentOS, SUSE and more) plus Windows from on-disk signals. — _Accurate identity without a running kernel._
  - **How:** CLI: `guestkit inspect disk.qcow2` reports OS, distro, version and init system; the same identity shows in the TUI Summary view and Python via `Guestfs.inspect_os()`.
- **Deep system enumeration** — Extracts packages, kernels, users, SSH config, services, timers, network, DNS, LVM, fstab, runtimes, containers, certificates, and cloud-init state. — _A complete inventory of the machine from bytes on disk._
  - **How:** CLI: run focused subcommands like `guestkit packages|services|users|network disk.qcow2`, or the full `guestkit inspect disk.qcow2` for everything in one pass.
- **Windows signal parsing** — Reads SAM/SECURITY registry hives to detect BitLocker, domain join, RDP, and driver gaps for Windows guests. Detects System Reserved / ESP boot volumes on multi-partition disks so BCD is not falsely reported missing. Samples hotfixes (HotFix key, `$NtUninstall*`, CBS.log) and VirtIO `.sys` presence for cutover planning. Offline OEM/Volume activation markers, remnant/ghost NICs, and static Tcpip configs feed migration warnings. Offline BitLocker `BootStatus` / FVE artifacts and VSS service inference feed Passport hard-blocks and snapshot readiness. — _See the Windows-specific blockers Linux tools miss._
  - **How:** CLI: `guestkit inspect disk.vmdk --profile windows-migration` parses SAM/SECURITY hives for BitLocker, domain join, RDP and driver gaps.
- **Pure-Rust engine, no legacy appliance tooling** — Partition tables, filesystem signatures, and evidence schema are parsed in Rust; only host NBD/loop is used for mount. — _No guestkit appliance, no fragile daemon - fewer moving parts._
  - **How:** Automatic on every command; add `--trace` (e.g. `guestkit inspect disk.qcow2 --trace`) to see the Rust parsers and the host NBD/loop mount that were used.

> Inspection is always read-only: your source image is never modified.

## 2. Migration Assurance

_Score boot readiness and generate hypervisor-aware fix plans before you cut over._

| Target | Boot analysis | Migration rules applied |
|---|---|---|
| kvm / proxmox / qemu | Proxmox/KVM | VirtIO, virtio-scsi/net, VMware Tools to qemu-ga |
| aws / azure / gcp / cloud | Cloud | cloud-init datasource, BYOL licensing |
| hyperv | Hyper-V | Hyper-V-specific boot checks |

- **Boot assurance score** — guestkit doctor predicts first-boot success on a target hypervisor with a 0-100 score, ranked blockers, and warnings. — _Answer 'will it boot?' before the weekend, not during it._
  - **How:** CLI: `guestkit doctor vm.qcow2 --target proxmox`. In the TUI press `d` on the Assurance tab; `t` cycles the target.
- **Root-cause --explain** — An inference engine traces each blocker back through a causal chain so you see why a VM would fail to boot. — _Fix the cause, not the symptom._
  - **How:** CLI: `guestkit doctor vm.qcow2 --target proxmox --explain` prints the causal chain behind each blocker.
- **Migration score + checklist** — guestkit migrate-plan applies target-specific rules for VirtIO drivers, cloud-init, VMware Tools removal, BitLocker, and SELinux relabel across eight targets. — _A tailored cutover checklist per destination platform._
  - **How:** CLI: `guestkit migrate-plan vm.vmdk --target proxmox` (add `-o json` to capture the checklist). Targets include kvm, proxmox, qemu, aws, azure, gcp, cloud, hyperv.
- **CI boot gate** — --fail-below sets an exit-code threshold so pipelines block any image that scores under your bar, JSON still emitted. — _Golden images that regress never reach production._
  - **How:** CLI: `guestkit doctor img.qcow2 --target proxmox -o json --fail-below 80` exits non-zero when the score drops below your bar while still emitting JSON.
- **Assured QEMU launch** — `guestkit-qemu` turns the same evidence + boot score into a VirtIO-aware QEMU definition and refuses to start on blockers, low scores, or missing UEFI firmware. — _Prove the disk boots under KVM before cutover weekend._
  - **How:** CLI: `guestkit-qemu plan vm.qcow2 --json` then `guestkit-qemu run vm.qcow2 --min-boot-score 80`. See [qemu-runtime.md](features/qemu-runtime.md).
- **Cutover Passport** — Versioned assurance artifact (scores, blockers, FixPlan digest, BitLocker hard-block, optional live attestation + Ed25519 sign). Transiva exports; h2kvm converts; GuestKit certifies. — _The gate MTV/virt-v2v cannot skip._
  - **How:** CLI: `guestkit passport emit vm.qcow2 --target kvm -o passport.json` then `guestkit passport verify passport.json --fail-below 80`. Web dock: **Passport**.
- **Windows day-0 pack** — Offline hostname, RDP, WinRM, domain→workgroup markers, timezone, DHCP/DNS, and static IP (by interface GUID) plans. — _Prep Windows guests without powering them on._
  - **How:** CLI: `guestkit plan generate win.qcow2 -p windows-domain-leave`, `-p windows-timezone --timezone UTC`, `-p windows-static-ip --interface-guid … --ip … --mask …`, `-p windows-dhcp` / `-p windows-dns`.
- **Policy-as-code** — guestkit policy check evaluates an expression DSL over evidence fields (e.g. bootability.score >= 80) or built-in CIS benchmarks. — _Codify sign-off criteria your whole team can trust._
  - **How:** CLI: `guestkit policy check vm.qcow2` evaluates the evidence DSL (e.g. `bootability.score >= 80`) or a built-in CIS benchmark.
- **Forensic diff** — guestkit forensic-diff compares two snapshots for config drift, suspicious persistence, and ransomware indicators. — _Prove what changed between golden and drifted._
  - **How:** CLI: `guestkit forensic-diff golden.qcow2 drifted.qcow2` compares two snapshots for drift, persistence and ransomware indicators.

## 3. Fix Plans & Repair

_Turn findings into reviewable, reversible, executable remediation - not blind edits._

- **Reviewable fix plans** — Findings become a structured plan of operations (file edits, package installs, service ops, SELinux, registry edits, Symlink/FileWrite) that you preview before anything runs. — _See every change before it happens._
  - **How:** CLI: `guestkit plan preview` shows every operation before it runs. In the TUI Assurance tab press `p` to preview the generated plan.
- **Day-0 canned plans** — Offline enablement without a full inspect pass: `windows-rdp`, `windows-hostname` (`--hostname`), `windows-winrm`, `linux-ssh` (optional `--user` + `--key`/`--key-file`), `linux-hostname`, `linux-grub`. Prefer `--skip-backup` for these low-risk registry/file plans.
  - **How:** `guestkit plan generate win.qcow2 -p windows-rdp -o rdp.yaml` then `guestkit plan apply rdp.yaml --vm win.qcow2 --yes --skip-backup`. Linux: `guestkit plan generate disk.qcow2 -p linux-ssh --user ubuntu --key-file ~/.ssh/id_ed25519.pub -o ssh.yaml`.
- **Rescue shortcuts** — Day-0 jobs without a plan file: Linux `enable-ssh`, `inject-ssh-key`, `set-hostname`, `reset-password`, `fix-fstab`, `check-grub`, `fix-grub`; Windows `enable-rdp`, `enable-winrm`, `set-timezone`, `set-hostname`, `reset-password` (AES/RC4 SAM NT-hash via SYSKEY, RunOnce fallback).
  - **How:** `guestkit rescue disk.qcow2 -o enable-ssh`; `guestkit rescue disk.qcow2 -o fix-grub`; `guestkit rescue win.qcow2 -o reset-password --user Administrator --password '…'` (needs `--features registry-write`).
- **Offline PackageInstall** — Stage `.rpm`/`.deb` from `GUESTKIT_PACKAGE_CACHE` for first-boot install; set `GUESTKIT_PACKAGE_FETCH=1` to download missing packages on the host first; optional `GUESTKIT_PACKAGE_MIRROR` for HTTP fallback (e.g. macOS).
  - **How:** `export GUESTKIT_PACKAGE_CACHE=~/pkgs GUESTKIT_PACKAGE_FETCH=1` then `guestkit plan apply plan.yaml --vm disk.qcow2 --yes`.
- **Offline service / command ops** — `ServiceOperation` enable/disable writes systemd wants; start/restart and `CommandExec` stage `guestkit-firstboot-live.service` when chroot cannot run them.
  - **How:** Preview shows staging notes; apply with `guestkit plan apply plan.yaml --vm disk.qcow2 --yes`.
- **Transactional boot repair** — guestkit repair --fix boot converts doctor blockers into a plan, applies it with backups, then re-scores to show the delta. — _Fix boot blockers offline and prove the score improved._
  - **How:** CLI: `guestkit repair vm.qcow2 --fix boot --dry-run` to preview, then `guestkit repair vm.qcow2 --fix boot`; re-run `guestkit doctor` to see the score delta.
- **Export to bash & Ansible** — Plans export as executable shell scripts, Ansible playbooks, JSON, or YAML for change control and runbooks. — _Hand ops a runbook your CAB can approve._
  - **How:** CLI: `guestkit migrate-plan vm.qcow2 --target kvm --export plan.yaml` (or export as bash/Ansible/JSON) for change control. TUI Assurance `e` exports YAML.
- **Backup & rollback** — guestkit plan apply creates timestamped backups (unless `--skip-backup`); plan rollback restores prior state, with dependency ordering and dry-run. — _Every change has an undo button._
  - **How:** CLI: `guestkit plan apply plan.yaml` writes timestamped backups; `guestkit plan rollback` restores prior state (both support `--dry-run`).
- **Automated hardening** — guestkit harden generates security-profile fixes for SSH, firewall, SELinux/AppArmor, and account posture. — _Ship hardened images without hand-editing configs._
  - **How:** CLI: `guestkit harden vm.qcow2` generates SSH, firewall, SELinux/AppArmor and account-posture fixes as a reviewable plan.
- **Offline agent injection** — repair --inject-agent writes a guest agent binary into the disk during migration prep, no boot required. — _The VM comes up already instrumented._
  - **How:** CLI: `guestkit repair vm.qcow2 --fix boot --inject-agent --agent-binary ./target/x86_64-unknown-linux-musl/release/guestkit`, or add `--inject-agent` to `migrate-plan --export`.

> Security teams generate plans; ops teams apply them - full separation of duties, in version control.

## 4. Security & Threat Analysis

_Audit posture, hunt for compromise, and prove compliance - all from the offline disk._

- **Security profile audit** — guestkit inspect --profile security scores SSH exposure, UID-0 users, firewall, SELinux/AppArmor, and kernel into a risk level. — _A ranked risk verdict per VM in seconds._
  - **How:** CLI: `guestkit inspect vm.qcow2 --profile security` returns a ranked risk verdict for SSH, UID-0 users, firewall, SELinux/AppArmor and kernel.
- **Secret & credential scan** — guestkit secrets sweeps the disk for exposed credentials and keys. — _Catch leaked secrets before an image ships._
  - **How:** CLI: `guestkit secrets vm.qcow2` sweeps the offline disk for exposed credentials and keys.
- **Malware & rootkit detection** — guestkit malware scans for rootkits and known-bad artifacts offline, where in-guest malware can't hide from the scanner. — _Inspect a suspect image without executing it._
  - **How:** CLI: `guestkit malware vm.qcow2` scans for rootkits and known-bad artifacts without executing the image.
- **CVE & patch analysis** — guestkit patch --check-cves maps installed packages to known vulnerabilities and missing security patches. — _See the VM's exposure without a live agent._
  - **How:** CLI: `guestkit patch vm.qcow2 --check-cves` (or `guestkit inventory vm.qcow2 --include-cves`) maps installed packages to known vulnerabilities and missing patches.
- **Compliance checking** — guestkit compliance and audit evaluate images against security standards with detailed reporting. — _Turn every VM into an audit artifact._
  - **How:** CLI: `guestkit compliance vm.qcow2` (or `guestkit audit vm.qcow2`) evaluates the image against security standards with a detailed report.
- **Threat hunting & IOC** — guestkit intelligence, hunt, and anomaly correlate indicators, detect anomalies, and surface suspicious persistence offline. — _Forensic triage on a dead disk, safely._
  - **How:** CLI: `guestkit intelligence vm.qcow2`, `guestkit hunt vm.qcow2`, and `guestkit anomaly vm.qcow2` correlate indicators and surface suspicious persistence.
- **Forensic timeline & reconstruction** — guestkit timeline and reconstruct build an incident timeline from multiple on-disk sources and visualize the attack path. — _Rebuild what happened without booting the evidence._
  - **How:** CLI: `guestkit timeline vm.qcow2` builds an incident timeline and `guestkit reconstruct vm.qcow2` visualizes the attack path.

## 5. Inventory & Reporting

_Produce SBOMs, license reports, and shareable documents from any image._

- **SBOM generation** — guestkit inventory emits a software bill of materials in SPDX or CycloneDX from the guest package set. — _Supply-chain inventory for every VM you run._
  - **How:** CLI: `guestkit inventory vm.qcow2 --format spdx` (or `cyclonedx`) emits a software bill of materials from the guest package set.
- **License compliance** — guestkit license inventories package licenses across the disk for compliance review. — _Know your license exposure before an audit asks._
  - **How:** CLI: `guestkit license vm.qcow2` inventories package licenses across the disk.
- **Self-contained HTML reports** — --export html builds an interactive, collapsible, print-friendly report with all CSS and JS embedded. — _Email a single file to any stakeholder._
  - **How:** CLI: add `--export html` to any inspect run, e.g. `guestkit inspect vm.qcow2 --export html` for a single self-contained file.
- **Git-friendly Markdown** — --export markdown produces version-controllable inventory documents for VM-configuration history. — _Track infrastructure drift in your docs repo._
  - **How:** CLI: `guestkit inspect vm.qcow2 --export markdown` produces a version-controllable inventory document.
- **Machine-readable output** — Most commands accept -o json or -o yaml for automation, monitoring, and jq/yq pipelines. — _Wire GuestKit straight into your tooling._
  - **How:** CLI: append `-o json` or `-o yaml` to most commands (e.g. `guestkit inspect vm.qcow2 -o json | jq`).
- **Fleet posture analysis** — guestkit fleet analyze scans a directory of images, clusters identical OS fingerprints, and flags snowflakes and low-score blockers. — _See fleet-wide drift at a glance._
  - **How:** CLI: `guestkit fleet analyze ./images/` clusters OS fingerprints and flags snowflakes and low-score images across a directory.

> --output (json/yaml/text) and --export (html/markdown) are mutually exclusive - run twice for both. PDF is produced via external tools (wkhtmltopdf, headless browser).

## 6. Planning & Optimization

_Reverse-engineer infrastructure-as-code, model cloud cost, and map dependencies from a disk._

- **Infrastructure-as-code blueprints** — guestkit blueprint generates Terraform, Ansible, Kubernetes, or Docker Compose definitions from what it finds on the image. — _Recreate a legacy VM as code you can redeploy._
  - **How:** CLI: `guestkit blueprint vm.qcow2 --format terraform` (also ansible/kubernetes/compose) regenerates the VM as redeployable code.
- **Cloud cost analysis** — guestkit cost profiles the workload and estimates run cost plus savings opportunities across AWS, Azure, and GCP. — _Price the migration before you commit to a cloud._
  - **How:** CLI: `guestkit cost vm.qcow2` estimates run cost and savings across AWS, Azure and GCP.
- **Dependency graph** — guestkit dependencies builds a package dependency graph with conflict, circular-dependency, and impact analysis. — _Understand blast radius before you change anything._
  - **How:** CLI: `guestkit dependencies vm.qcow2` builds the package dependency graph with conflict, circular and impact analysis.
- **Disk format conversion** — guestkit convert transcodes images between the six supported formats using qemu-img. — _Reformat once, migrate anywhere._
  - **How:** CLI: `guestkit convert vm.vmdk --output vm.qcow2 --format qcow2 --compress` transcodes between the six supported formats via qemu-img.
- **Smart recommendations** — guestkit recommend and predict surface tuning and remediation guidance grounded in the evidence snapshot. — _Actionable next steps, not just raw data._
  - **How:** CLI: `guestkit recommend vm.qcow2` (and `guestkit predict vm.qcow2`) surface tuning and remediation guidance from the evidence snapshot.
- **Performance profile** — --profile performance flags swappiness, I/O scheduler, mount options, and network tuning opportunities. — _Baseline and tune before cutover._
  - **How:** CLI: `guestkit inspect vm.qcow2 --profile performance` flags swappiness, I/O scheduler, mount options and network tuning.

## 7. Interactive Workspaces

_A carbon TUI, a file explorer, and a shell for hands-on offline investigation._

- **Carbon-themed TUI** — guestctl tui opens a k9s-style dashboard with grouped views, vim keys, a command palette, and glass/transparency themes. — _Explore a VM visually without leaving the terminal._
  - **How:** TUI: `guestctl tui vm.qcow2` opens the k9s-style dashboard; navigate with vim keys and the command palette.
- **Assurance view parity** — The TUI Assurance tab runs doctor, cycles targets (kvm/proxmox/aws), previews fix plans, and exports YAML - reusing the CLI engine on one mount. — _Full assurance workflow, keyboard-driven._
  - **How:** TUI: in `guestctl tui vm.qcow2` press `a` for the Assurance tab, then `d` run doctor, `t` cycle target (kvm/proxmox/aws), `p` preview fix plan, `e` export YAML.
- **Interactive file explorer** — guestkit explore browses partitions and files in place with view, info, filter, sort, and hidden-file toggles. — _Grep-free spelunking through a cold disk._
  - **How:** CLI/TUI: `guestkit explore vm.qcow2` browses partitions and files with view, info, filter, sort and hidden-file toggles.
- **Guest shell & REPL** — guestkit shell and interactive give ls/cat/grep/find over the mounted image plus a scriptable session. — _Familiar Unix muscle memory on any VM._
  - **How:** CLI: `guestkit shell vm.qcow2` (or `guestkit interactive vm.qcow2`) gives ls/cat/grep/find over the mounted image plus a scriptable session.
- **Fleet & compare modes** — --fleet browses a directory of images with a sidebar; --compare diffs two VMs side by side in the dashboard. — _Spot the odd VM out across a set._
  - **How:** TUI: `guestctl tui vm.qcow2 --fleet ./images/` for a fleet sidebar; `guestctl tui vm.qcow2 --compare other.qcow2` diffs two VMs side by side.
- **Global search & jump** — Cross-view search finds packages, boot blockers, and migration items; a grouped jump menu navigates every view. — _Find any signal without knowing which tab holds it._
  - **How:** TUI: inside `guestctl tui`, use cross-view search to find packages, boot blockers or migration items, and the grouped jump menu to navigate any view.
- **AI copilot Q&A** — guestkit ai answers natural-language questions grounded in the evidence snapshot, with pluggable LLM backends (OpenAI, Anthropic, xAI, or local Ollama). — _Ask a VM what's wrong and get an evidence-backed answer._
  - **How:** CLI: `guestkit ai vm.qcow2 "why won't this boot?"` answers grounded in the evidence snapshot (build with `--features ai`; backends: OpenAI, Anthropic, xAI, or local Ollama).

## 8. Guest Agent & Live Control

_Run inside the guest - or reach it host-mediated - even when there's no guest network._

- **In-guest agent** — guestkit agent runs like qemu-guest-agent over virtio-serial, reusing the same evidence and fix-plan schema as offline mode. — _One model for cold-disk and live guests._
  - **How:** Run `guestkit agent` inside the booted guest; it serves the same evidence and fix-plan schema over the virtio-serial channel `com.zyvor.guestkit.0`.
- **`guestkit qga` (dump virsh)** — Speaks the QGA unix socket directly; drop-in for `virsh qemu-agent-command`. zyvor-api uses the same ladder and only falls back to virsh when `GUESTKIT_ALLOW_VIRSH=1`. — _Live guest ops without libvirt-client in the path._
  - **How:** CLI: `guestkit qga --execute guest-ping` (auto-discovers the socket) or pin `--socket …`. Map: [virsh-to-guestkit.md](user-guides/virsh-to-guestkit.md).
- **Transport ladder** — The Guest Control Fabric auto-selects the best path per VM - virtio-serial, QGA exec, QGA builtin, push cache, offline disk, or console. — _Guest control that never depends on guest networking._
  - **How:** Host bridge: `guestkit agent-proxy --socket /var/lib/libvirt/qemu/channel/target/$VM/com.zyvor.guestkit.0 --listen 127.0.0.1:8765` auto-selects the best path per VM.
- **Snapshot quiesce** — Freeze and thaw guest filesystems (fsfreeze) for application-consistent snapshots, plus soft reboot and graceful shutdown. — _Clean snapshots without crash-consistency risk._
  - **How:** Agent RPC via the proxy (e.g. `curl http://127.0.0.1:8765/freeze` / `/thaw`) issues fsfreeze, soft reboot and graceful shutdown for consistent snapshots.
- **Live remediation with approval** — Restart failed units, collect support bundles, and run fix plans - policy-gated with JIT approval workflows. — _Safe, audited guest actions at fleet scale._
  - **How:** Agent RPC / worker jobs run fix plans and restart failed units under JIT approval, e.g. `curl -s http://127.0.0.1:8765/doctor | jq .` then submit an approved plan.
- **mTLS & signed updates** — Agents bootstrap client certs, push heartbeats over mTLS, and self-update from Ed25519-signed, SHA256-verified bundles. — _A hardened, tamper-evident guest agent._
  - **How:** Agents bootstrap client certs at enrollment and self-update from Ed25519-signed, SHA256-verified bundles; configured through the agent enrollment/config, not a per-run flag.
- **Deep Linux health** — Component scores for boot, systemd, network, DNS, storage, and security via systemd D-Bus, journald, /proc, and PSI pressure. — _Root-cause the failed unit from journal correlation._
  - **How:** Agent RPC: `curl -s http://127.0.0.1:8765/doctor | jq .` returns component scores (boot, systemd, network, DNS, storage, security) from D-Bus, journald, /proc and PSI.

> Deep guest intelligence targets Linux; Windows uses virtio-win and a scheduled-task updater today, with a native agent MSI scaffolded.

## 9. KubeVirt & Platform

_Boot-inspect stopped VMs in-cluster and drive it all from a self-hosted web console._

- **Offline boot-inspect for stopped VMs** — zyvor-api resolves a stopped VM's root PVC and runs GuestKit's boot-inspect evidence collection in-process, returning fstab validity, bootloader, and cloud-init state. — _Assurance for halted VMs without booting them._
  - **How:** Web console: open a stopped VM and run Boot Inspect (zyvor-api resolves the root PVC and calls GuestKit's `run_boot_inspect` library API directly — not a separate CLI command); available via the `/api/` boot-inspect endpoint.
- **Zeus VM Tools** — A Kubernetes-native guest agent with cloud-init, QGA, ISO, and airgap install paths plus VMToolsPolicy auto-install/upgrade reconciliation. — _The VMware Tools equivalent for KubeVirt._
  - **How:** Apply a `VMToolsPolicy` resource (or enable it from the web console) to auto-install/upgrade the KubeVirt guest agent via cloud-init, QGA, ISO or airgap path.
- **Web console** — Self-hosted zyvor-ui + zyvor-api + guestkit-worker ship as public GHCR images and a Helm chart, backed by a Redis job queue. — _A team-facing UI over the same engine._
  - **How:** Browse to http://localhost:8088 and sign in with `admin` / `Admin@321` (change immediately). The nginx front-end proxies `/api/` to zyvor-api. Inventory tabs, Assurance (doctor/plan/passport/repair), Profiles, and Files browse map to `POST /api/v1/vms/:id/{inspect,doctor,migration-plan,passport,repair-plan,profile,explore}`. Lab HTTPS: `./scripts/deploy-ui-remote.sh` with `--api-upstream`. See [Using the Dashboard](user/using-the-dashboard.md).
- **Python bindings** — [`hypersdk-guestkit`](https://pypi.org/project/hypersdk-guestkit/) on PyPI: Guestfs handle + assurance APIs (`run_doctor`, `run_migrate_repair`) for programmatic inspection and offline repair.
  - **How:** Python: `from guestkit import Guestfs` then `g = Guestfs(); g.add_drive("vm.qcow2"); g.launch(); g.inspect_os()` — a GuestKit-style API with 100+ methods.
- **h2kvm pipeline** — Pairs with h2kvm for VMware-to-KVM conversion, sitting in the wider Transiva to GuestKit to v9s to PacketWolf flow. — _One assurance gate inside a full migration pipeline._
  - **How:** Run h2kvm for the VMware-to-KVM conversion and call `guestkit doctor`/`migrate-plan` as the assurance gate in the same pipeline.
- **Pluggable auth** — The web stack supports JWT, local login, and OIDC/SAML hooks with JWKS-verified ID tokens. — _Wire the console into your existing identity._
  - **How:** Web console: go to **Settings** to enable OIDC/SAML (JWKS-verified ID tokens) or keep JWT/local login; rotate the seeded password and `JWT_SECRET` first.
- **KubeVirt manifest generation** — Emits ready-to-apply DataVolume and VirtualMachine YAML with CDI import URLs, storage class, and CPU/memory sized from the migration plan. — _From disk image to running KubeVirt VM in one manifest._
  - **How:** CLI: `guestkit migrate-plan vm.qcow2 --target kvm` emits ready-to-apply DataVolume and VirtualMachine YAML with CDI import URL, storage class and sized CPU/memory; also exportable from the web console.
- **Cloud image sources** — Inspects images directly from S3, GCS, and Azure Blob URIs, resolving them to a local path on the fly. — _Assess VMs where they already live in object storage._
  - **How:** CLI: point any command at an object-storage URI, e.g. `guestkit inspect s3://bucket/vm.qcow2` (also gs:// and Azure Blob), and it resolves to a local path on the fly.

> In-cluster boot-inspect needs a privileged pod or node disk access plus get/list RBAC on VMs, VMIs, PVCs, and PVs. A running VM returns VM-spec heuristics - offline disk access is for stopped VMs.

## 10. QEMU / VirtIO runtime

_Turn migration evidence into an executable, assurance-gated QEMU definition._

- **Evidence → QEMU plan** — `guestkit-qemu plan` reuses `collect_assurance_data` to pick architecture, machine type, disk format, VirtIO devices, firmware needs, and a safe argv vector. — _One inspection path for score and launch config._
  - **How:** CLI: `guestkit-qemu plan vm.qcow2 --memory-mb 8192 --vcpus 4 --json`.
- **Gated run** — Launch refuses boot blockers, scores below `--min-boot-score`, or UEFI without pflash unless `--allow-unready`. — _No silent start of unready cutovers._
  - **How:** CLI: `guestkit-qemu run vm.qcow2 --min-boot-score 80 --qmp-socket /run/guestkit/vm.qmp`.
- **QMP day-2** — status / pause / resume / balloon / powerdown over a Unix QMP socket. — _Control the VM after assured start._
  - **How:** CLI: `guestkit-qemu qmp --socket /run/guestkit/vm.qmp status`.
- **Host networking stays outside** — User-mode (SSH on loopback) by default; TAP/bridge attach only to already-provisioned host devices. — _GuestKit inspects; the platform owns host net._

Full guide: [qemu-runtime.md](features/qemu-runtime.md).

## 11. Deployment & Editions

_Install in one command; run the full open-source stack; scale with Enterprise support._

- **cargo install** — cargo install guestkit installs `guestkit`, `guestctl`, and `guestkit-qemu`. — _From zero to inspecting in one line._
  - **How:** CLI: `cargo install guestkit` installs the CLI, TUI, and QEMU runtime binaries.
- **Run from GHCR** — Prebuilt public images (zyvor-ui, zyvor-api, guestkit-worker) come up via docker compose with no docker login. — _Stand up the whole console in minutes._
  - **How:** Docker: `docker compose -f deploy/docker-compose.ghcr.yml up -d` brings up zyvor-ui/zyvor-api/guestkit-worker at http://localhost:8088 with no docker login.
- **Helm & remote deploy** — A Helm chart for clusters plus scripted remote deploy for Docker hosts. — _Ship it where your fleet already lives._
  - **How:** Cluster: `helm install` the chart (provisions Postgres/Redis/MinIO); for Docker hosts use the scripted remote deploy under `scripts/`.
- **Full open-source stack** — CLI, TUI, Python bindings, assurance APIs, web console, and KubeVirt hooks are all Apache-2.0 in the repo - Enterprise adds support, not features. — _Nothing core is withheld from the open source._
  - **How:** Clone the repo (`git clone https://github.com/zyvorai/guestkit`) — CLI, TUI, Python bindings, assurance APIs, web console and KubeVirt hooks are all Apache-2.0.
- **Enterprise programs** — SLA, air-gapped deployment packages, guided playbooks, and fleet automation for 100+ VM and regulated migrations. — _Backed help for VMware-exit programs at scale._
  - **How:** Contact the account team at info@zyvor.dev for SLA, air-gapped packages, guided playbooks and fleet automation.

> The default eval compose stack runs without authentication - do not expose it beyond localhost. Use the production template and turn auth on before any network exposure.

## Getting started

1. **Install** — Run cargo install guestkit to get the guestkit CLI and guestctl TUI, or pull the web stack from ghcr.io/zyvorai.
2. **Score boot readiness** — guestkit doctor vm.qcow2 --target proxmox --explain returns a 0-100 boot assurance score with ranked blockers and root-cause chains.
3. **Export a fix plan** — guestkit migrate-plan vm.vmdk --target proxmox --export plan.yaml writes an executable, reviewable migration fix plan.
4. **Explore interactively** — guestctl tui vm.qcow2 opens the carbon TUI with the Assurance workspace and fix-plan preview.
5. **Gate your pipeline** — guestkit doctor img.qcow2 --target proxmox -o json --fail-below 80 fails CI on any image that regresses below your bar.

> **Good to know:** GuestKit runs on Linux hosts and needs host tooling (losetup, qemu-nbd, and qemu-img for conversion); mount and in-cluster boot-inspect operations require root or a privileged pod. Offline disk analysis targets stopped VMs - running VMs return VM-spec heuristics or need the live guest agent. Deep guest intelligence is Linux-first; the native Windows agent MSI is scaffolded, and Windows relies on virtio-win today. LLM-assisted features require building with --features ai; deterministic intelligence works without it. Reporting notes: --output and --export are mutually exclusive, HTML reports truncate to 100 packages, and PDF is produced via external tools.

---
_GuestKit is developed by ZyvorAI Labs (Apache License 2.0). Contact **info@zyvor.dev**._
