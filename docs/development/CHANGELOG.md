# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Web Image Vault TUI parity** — OSS `deploy/ui` renders inspect inventory tabs, Assurance (doctor / migration plan / passport / repair preview + gated apply), Profiles/Issues, and read-only Files browse (`guestkit.explore`). Docs: [using-the-dashboard.md](../customer/using-the-dashboard.md).
- **`POST /api/v1/vms/:id/profile`** and **`POST /api/v1/vms/:id/explore`** — enqueue existing/new worker ops; repair-plan accepts `dry_run` query (default true).
- **OpenAPI** documents profile, explore, and repair `dry_run`.

### Changed
- **Suite positioning** — GuestKit certifies/repairs disks; **FluxVM** runs and
  manages VMs (network, cloud-init, TTL) and is the host-local **libvirt/virsh
  replacement**. `guestkit vm` stays a minimal lab/CI smoke path. Docs:
  [vm-runtime.md](../features/vm-runtime.md),
  [virsh-to-guestkit.md](../user-guides/virsh-to-guestkit.md).
- **Inspect worker summary** — larger package/service samples (200/100) for UI lists.

### Fixed
- **open-vm-tools false positive** — detect OSS `open-vm-tools` separately from
  proprietary `vmware-tools` so BOOT-006/BOOT-010/MIG-G-005 no longer warn on
  Ubuntu cloud images that ship `vmware-toolbox-cmd` via open-vm-tools.
- **`guestkit vm` runtime dirs** — deploy writes `/etc/tmpfiles.d/guestkit.conf`
  so `/run/guestkit/vms` is recreated owned by the deploy user after reboot.
- **BOOT-003 false positive on Ubuntu cloud images** — mount `/boot` and
  `/boot/efi` from fstab (`LABEL=`/`UUID=`/`PARTUUID=`). Previously only the
  rootfs was mounted, so a separate BOOT partition looked empty and passport
  handoff hard-blocked a bootable disk.
- **`guestkit gate --image`** — when the disk directory is not writable (e.g.
  `/var/lib/libvirt/images`), write the temporary passport under `$TMPDIR`
  instead of failing with Permission denied.

### Added
- **`guestkit vm`** — GuestKit-native local QEMU lifecycle (`define` / `plan` /
  `start` / `list` / `status` / `shutdown` / `destroy` / `undefine`) with
  assurance gate and QMP day-2 ops. No libvirt XML. Docs:
  [vm-runtime.md](../features/vm-runtime.md).
- **Remote deploy** installs `virtctl-guestkit`, `kubectl-guestkit`, and
  `guestkit-qemu` alongside `guestkit`, and seeds `/var/lib/guestkit/vms` +
  `/run/guestkit/vms` for `guestkit vm`.
- **`virtctl-guestkit guestfs`** — drop-in for `virtctl guestfs`. Creates a
  short-lived GuestKit pod on a PVC (`/disk` or `/dev/vda`), no libguestfs
  appliance. Also `inspect` / `doctor --vm` / `rescue`. Uses `kubectl` (no new
  crate deps). Docs: [virtctl-guestkit.md](../features/virtctl-guestkit.md).
- **Cutover bundle** — `gate`, `selinux-relabel`, `sysprep`, `bitlocker`,
  `cloud-profile`, `policy rego`, `virtio-initramfs`, `agent-sign`;
  virtctl `resolve`/`gate` (hostDisk only).
- **Cloud cutover profiles** — `guestkit cloud-profile aws|azure|gcp|openstack`
  and `policy check -b aws`. cloud-init + boot-score rules, no cloud API calls.
- **`guestkit policy rego`** — in-process `deny[msg]` subset (+ optional `opa eval`).
  Example: `policies/cutover.rego`.
- **`guestkit cloud-init`** — offline datasource pin (`aws`/`azure`/`gcp`/`openstack`/`nocloud`)
  plus optional NoCloud user-data/meta-data seed. Also
  `plan generate -p cloud-init-aws`. Closes the migrate-plan "reconfigure
  datasource" item with a real FileWrite plan.
- **`guestkit sbom-diff`** — compare SPDX, CycloneDX, or GuestKit inventory
  JSON; `--fail-on-drift` for CI. `forensic-diff` accepts `--sbom-old/--sbom-new`.
- **Passport Action extras** — optional rescue dry-run (`--export-plan`), SPDX
  emit, and `passport handoff` in the composite `action.yml`. New
  `.github/actions/rescue-dry-run`.
- **`guestkit img`** — qemu-img info/check/snapshot/resize/rebase/commit with
  GuestKit JSON errors (`GUESTKIT_QEMU_IMG` override).
- **`guestkit domain-disks`** — parse libvirt XML or KubeVirt VM/VMI YAML for
  disk sources (replaces `virsh dumpxml | grep source`).
- **`guestkit virtio-win list|plan`** — resolve a virtio-win tree
  (`--tree` / `$GUESTKIT_VIRTIO_WIN`) and emit the `migrate-repair --apply` hint.
- **`guestkit firstboot`** — cutover attestation JSON: offline doctor + live
  QGA ping + virtio plan + domain disks; `--fail-below` for CI.
- **`guestkit fleet quarantine`** — hold disks below `--threshold` (default 80)
  out of the conversion wave; `--fail` for CI.
- **`guestkit passport handoff`** — verify a Cutover Passport and write
  `*.handoff.yaml` for `h2kvmctl --passport`. Refused passports do not convert.
- **`virtctl-guestkit` / `kubectl-guestkit` plugin** — `virtctl guestkit doctor|passport|handoff`
  wraps the GuestKit CLI so OpenShift/KubeVirt operators never touch virsh.
- **QEMU/VirtIO runtime** (`src/qemu/`, `guestkit-qemu` binary) — turns GuestKit
  evidence + boot assurance into a declarative `QemuVm` plan and launches QEMU
  only when blockers/score/UEFI firmware gates pass (`plan` / `run` / `qmp`).
  Safe argv construction (no shell), VirtIO block/SCSI/net/balloon/rng/vsock/GPU,
  OVMF/AAVMF, user/TAP/bridge networking (host TAP/bridge provisioning stays
  outside GuestKit). Docs: [qemu-runtime.md](../features/qemu-runtime.md).
- **`guestkit qga`** — drop-in for `virsh qemu-agent-command`; speaks the QGA
  unix socket directly (auto-discovers libvirt / KubeVirt sockets). Docs:
  [virsh-to-guestkit.md](../user-guides/virsh-to-guestkit.md).

### Changed
- **Dump `virsh` from the live GuestKit path.** `zyvor-api` no longer
  `kubectl exec`s `virsh qemu-agent-command` inside virt-launcher. It
  discovers the QGA unix socket and speaks the QGA wire format through
  `guestkit qga` / python / perl / socat / nc. `virsh` is an explicit
  opt-in via `GUESTKIT_ALLOW_VIRSH=1`.
- Docs and MIG-L-009 no longer tell operators to use `virsh console`
  or `virsh qemu-agent-command`.

### Fixed
- **`guestkit qga_client`**: import `std::os::unix::fs::FileTypeExt` so
  `is_socket()` compiles on Unix.

## [1.1.0] - 2026-08-31

### Added
- **Python assurance bindings** (`guestkit.pyi`, `src/python.rs`) — native PyO3 wrappers for the migration assurance engine:
  - `run_doctor(image, target="kvm")` — bootability score, blockers, evidence
  - `run_boot_inspect(image, target="kvm")` — OS release, fstab, bootloader summary
  - `run_migrate_plan(image, target="kvm")` — hypervisor-aware fix plan
  - `run_repair_plan(image, dry_run=True)` — repair plan with before/after scores
  - `run_migrate_repair(image, apply=False)` — primary offline fix path used by **h2kvm**
- **h2kvm integration** — `h2kvm.core.guestkit_client` delegates to these APIs; VMCraft removed from h2kvm
- Documentation: [python-bindings.md](../user-guides/python-bindings.md), [hyper2kvm-integration.md](../features/hyper2kvm-integration.md)

### Changed
- Python package remains **`hypersdk-guestkit`** on PyPI; wheel artifact: `hypersdk_guestkit-*.whl`
- Assurance APIs return structured `dict` results matching CLI JSON output
- **Published:** PyPI [`hypersdk-guestkit==1.1.0`](https://pypi.org/project/hypersdk-guestkit/1.1.0/), GitHub Release binaries, crates.io

### Fixed
- Python 3.13+ builds: document `PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1` for maturin when needed

## [1.0.1] - 2026-08-15

### Fixed
- **`guestkit-worker`'s Docker release build failed for every release past
  the `0.x` line** — `crates/guestkit-worker/Cargo.toml` pinned its own
  `guestkit` path dependency to `version = "0.3.3"`; the worker's
  `Dockerfile` never copies a `Cargo.lock` in, so each build freshly
  resolves dependencies and cargo enforces the version requirement even
  against a `path` dependency — `1.0.0` didn't satisfy `^0.3.3` and the
  `Publish GHCR Release Images` job in `v1.0.0`'s release run failed.
  Dropped the version pin (path-only, matching `zyvor-api` and
  `zyvor-guest-agent`'s existing pattern for the same in-workspace
  dependency).

## [1.0.0] - 2026-08-15

### Added
- **GitHub Action for the Passport CI gate** (`action.yml`) — reusable
  composite action wrapping `doctor → migrate-plan → passport emit →
  passport verify` as a single CI step; installs a checksum-verified
  release binary, no build step. Dogfooded against a real disk image by
  `.github/workflows/passport-gate-demo.yml` on every change. See
  `docs/devops/01-passport-ci-gate.md`.
- **Native OpenAI tool-calling for the AI copilot** (`src/ai/rig_tools.rs`)
  — rig-core `AgentBuilder`/`multi_turn` with real JSON-schema tool
  definitions, replacing regex/JSON-scraped completion text for OpenAI.
  xAI/Anthropic/Ollama still use the original text-instructed loop.
- **Cross-run AI memory** (`src/ai/memory.rs`) — a repeated `doctor --ai`/
  `migrate-plan --ai` run against the same VM folds a summary of prior
  findings into the query, capped at the last 20 runs.
  `GUESTKIT_AI_MEMORY_DIR`/`GUESTKIT_AI_MEMORY=0` to relocate/disable.
- **MCP server for the AI copilot** (`src/ai/mcp.rs`, `--features mcp`) —
  `guestkit mcp-serve <disk> [--target <target>]` exposes the same 6
  read-only evidence tools over stdio to Claude Desktop / other MCP
  hosts, independent of guestkit's own agent loop.
- **`guestkit fleet wave-plan`** (`src/fleet/wave.rs`) — orders a fleet's
  disk images into dependency-aware migration waves (DB-role priority +
  NFS storage-dependency edges, Kahn's-algorithm topological sort,
  cycles reported rather than dropped or arbitrarily ordered).
- **`guestkit fleet watch`** (`src/fleet/baseline.rs`) — scheduled drift
  monitoring: diffs each VM's current evidence against a stored golden
  baseline (first run establishes it), `--fail-on-drift` for pipeline
  gating. Includes a Kubernetes `CronJob` template
  (`deploy/helm/zyvor/templates/fleet-drift-watch-cronjob.yaml`) as the
  reference scheduled-invocation path.
- **Helm chart CI** — `ci.yml`'s new `helm-chart` job runs `helm lint` and
  `helm template` against `deploy/helm/zyvor`, which previously had zero
  CI coverage, across default values, every optional PVC-backed feature
  enabled at once, hostPath-backed persistence, and all three real
  deployment overlays (`values-ci.yaml`, `values-k3s.yaml`,
  `values-prod.yaml`).
- **Manual-dispatch workflow for NBD-dependent tests**
  (`self-hosted-nbd-tests.yml`) — runs the full test suite with none of
  `ci.yml`'s NBD skips, for self-hosted runners with working loop/NBD
  support. `workflow_dispatch` only, deliberately never wired to
  `pull_request`/`push`.

### Fixed
- **k3s E2E's `zyvor-api` pod crash-looped on every run** — the Helm chart's
  default `zyvorApi.agentMtls.enabled: true` requires `AGENT_BOOTSTRAP_TOKEN`
  (zyvor-api refuses to start otherwise: "AGENT_MTLS_BIND_ADDR is set but
  AGENT_BOOTSTRAP_TOKEN is unset"), but the Deployment template only ever
  wired that env var — and the Secret holding it — inside the
  `zyvorApi.auth.enabled` block. `values-ci.yaml` (mTLS on, full auth off)
  hit exactly that gap; `values-prod.yaml` masked it by having both auth and
  a token on together, and `values-k3s.yaml` worked around it by disabling
  mTLS outright (its own comment already described the bug). Decoupled the
  `zyvor-api-auth` Secret and `AGENT_BOOTSTRAP_TOKEN` env var from
  `auth.enabled` — gated only on the token itself being set, matching what
  `zyvor-api`'s own config validation actually requires — and set a
  CI-only placeholder token in `values-ci.yaml` so the E2E job now exercises
  the mTLS path instead of crash-looping. `ci.yml`'s Helm Chart job now also
  `helm template`s all three real overlays (`values-ci.yaml`,
  `values-k3s.yaml`, `values-prod.yaml`) so a rendering break here is caught
  without needing a live k3s cluster.
- **k3s E2E multi-round debugging** — the job was failing on every run;
  fixing it required peeling through several layers, each masking the
  next:
  - `poll_job` (`deploy/scripts/e2e-smoke.sh`) only recognized
    `"completed"` as terminal, so a `"failed"` job status looked
    identical to "still pending" for the full 5-minute poll budget, and
    the worker's own error message (`live_status.error`) was never
    printed. Now treats `failed`/`cancelled`/`timeout` as terminal and
    prints the error immediately.
  - `install-k3s-ubuntu.sh` never got the loop/NBD device setup
    `ci.yml` needed earlier this session (`guestkit-worker`'s pod
    bind-mounts the host's `/dev`, so the same root:disk-0660-node /
    EACCES-looks-like-timeout issue applies here too). Added it —
    confirmed live afterward: `inspect`/`doctor` both complete in one
    poll with a real bootability score. This specific k3s stack path
    does not hit the deeper NBD-attach limitation `ci.yml`'s plain
    `cargo test` job still has to skip around.
  - `curl -sf` swallows the response body on any non-2xx status, so the
    next failure (`provision`) looked like an empty response
    (`Expecting value: line 1 column 1`) instead of a real API error.
    Added a `curl_or_die` helper (splits HTTP status from body via
    curl's `-w`, prints both on failure).
  - That revealed a real HTTP 500: `"No operating system found in disk
    image"` from `provision` (`POST /vms/{id}/provision`, which mounts
    the disk *synchronously in zyvor-api's own process*), on the same
    image `doctor` had just inspected successfully. Suspected (and
    partially fixed) an unawaited async `migration-plan` job racing
    provision's own mount via `NbdDevice::find_available_device`
    (`src/disk/nbd.rs`) — that function does check device availability
    and connect as two separate, unlocked steps with no cross-process
    coordination, a real bug now flagged with a code comment — but
    serializing migration-plan before provision did **not** fix it,
    disproving the race as *this* failure's cause.
  - `provision_vm`'s `.map_err(|e| ApiError::internal(e.to_string()))`
    (`crates/zyvor-api/src/routes/vms.rs`) only shows anyhow's outermost
    `.context()` layer via plain `Display`. Changed to `format!("{e:#}")`
    (alternate Display, full chain) for `provision_vm`'s three
    `guestkit`/`export::kubevirt`-derived `map_err` calls — correct and
    worth keeping, but the next run's error was *still* byte-identical to
    before, because there was no chain to reveal: `mount_all_ro`
    (`src/cli/commands/mod.rs`) returns `Option<String>`, not
    `Result<String>`, and `.context("No operating system found in disk
    image")` on a `None` (anyhow's `Context` impl for `Option`) produces
    an error with *no* wrapped source at all — the context message
    genuinely is the entire error. Left the other ~90
    `.map_err(|e| ApiError::internal(e.to_string()))` sites in the crate
    alone; most wrap simple error types (`serde_json`, `std::io`) where
    `.to_string()` isn't lossy.
  - The real swallowed information was one layer further down:
    `mount_all_ro` calls `g.inspect_os().unwrap_or_default()` —
    `inspect_os()` failing for *any* reason (guestfs launch issue, mount
    error, permission problem) collapses to the identical empty-roots
    `None` as "genuinely no OS found," discarding whatever `inspect_os`'s
    real error was before it could reach any context message. Changed to
    log the real error (`log::warn!`) before discarding it. Didn't widen
    `mount_all_ro`'s `Option<String>` return type to `Result` — it's used
    across 9 files where callers only ever branch on `Some`/`None`, and
    that ripple is out of scope for this investigation. Widened the
    default `EnvFilter` (was "nothing enabled" without `RUST_LOG` set,
    now falls back to `warn` globally) so `mount_all_ro`'s new
    `log::warn!` — and any other `log::*!` from guestkit's dependency
    graph — actually reaches the pod's logs. First attempt at this also
    added an explicit `tracing_log::LogTracer::init()` call, reasoning
    that `zyvor-api` only sets up a `tracing` subscriber and guestkit
    logs through the plain `log` facade — **wrong, and a real
    regression**: `tracing-subscriber`'s "tracing-log" feature (on by
    default) already bridges `log` into the subscriber as part of
    `.init()`, so the explicit call double-registered the global `log`
    logger and panicked at startup with `SetLoggerError`, crash-looping
    `zyvor-api` again. Confirmed via `kubectl logs` from the next E2E
    run (once the "dump pod logs on failure" step below existed to
    capture it) and reproduced in an isolated 10-line binary before
    re-pushing — removed the explicit `LogTracer::init()` call and the
    now-unneeded direct `tracing-log` dependency; the isolated repro
    confirmed `log::warn!` still reaches the subscriber correctly
    without it.
  - Also: nothing in `k3s-e2e.yml` ever captured pod logs on failure —
    every fix in this list up to this point was diagnosed purely from
    HTTP response bodies, each requiring a full ~20-40min re-run just to
    test. Added a failure-only step dumping `kubectl logs` (all
    containers, prefixed by pod) for every deployed component, plus
    `get pods -o wide` and `describe pods`.
  - That finally showed it: no panic, no error, no `mount_all_ro`
    warning — `inspect_os()` genuinely returned an empty root list.
    `validate_root_partition`/`validate_initrd_boot_partition`
    (`src/guestfs/inspect.rs`) treat mount/extraction failures as
    "not a valid root" *by design*, not as errors — a real mount
    failure and "genuinely no OS" are indistinguishable at that layer
    on purpose (LVM volumes on read-only NBD devices can legitimately
    fail to mount for benign reasons). `validate_initrd_boot_partition`
    is the cirros-cloud-image path — root filesystem lives inside the
    initrd, not on a directly-mountable partition — and shells out to
    `zcat <initrd> | cpio -t` to look inside it.
    `crates/zyvor-api/Dockerfile` never installed `cpio` (or `gzip`),
    unlike `crates/guestkit-worker/Dockerfile`'s otherwise-identical
    package list, which does. That shell command silently failing
    inside `zyvor-api`'s container is why `provision` (`run_migrate_plan`
    called synchronously in zyvor-api's own process) couldn't find an
    OS on exactly the image `doctor`/`inspect` (via `guestkit-worker`,
    which has `cpio`) found one on every time. Added `cpio`/`gzip` to
    `zyvor-api`'s Dockerfile, matching `guestkit-worker`'s package list.
    Confirmed live afterward: `provision` succeeds, generating a real
    PVC-referencing manifest — this was the fix. The remaining unguarded
    `curl -sf | python3` call sites past `provision` (`/config`,
    `/kubevirt/vms`, `/kubevirt/namespaces`, `/vmtools/coverage`,
    `/vmtools/policy`, `/storage/roots`, the `E2E_KUBEVIRT` cluster-inspect
    calls) got the same `curl_or_die` treatment for consistency.
  - One more layer, different in kind from everything above:
    `/vmtools/coverage` now fails with a real, visible error —
    `kube list virtualmachines: ... 404 page not found`. `values-ci.yaml`
    sets `kubevirt.enabled: true` (zyvor-api/worker assume the KubeVirt
    CRDs exist), but neither `install-k3s-ubuntu.sh` nor
    `deploy-remote-k3s.sh` ever installed the KubeVirt operator that
    registers them — this repo already has that install (pinned
    `v1.4.0`, operator + CR manifests, tolerant wait) in
    `deploy/scripts/kind-kubevirt-quickstart.sh`, just never ported to
    the script this CI workflow actually uses. Ported it. GitHub-hosted
    runners have no `/dev/kvm` (no nested virtualization), so
    `virt-handler` won't reach fully healthy and starting a real
    `VirtualMachineInstance` still won't work here — but the CRDs
    register and the `kubevirt.io/v1` API group routes real (empty)
    responses instead of 404 as soon as the operator applies them, which
    is all this job's default (non-`E2E_KUBEVIRT`) path needs.
- **Main CI (`ci.yml`) had been broken for a while** — `journal-native`
  (a *default* feature) needs `libsystemd-dev`, missing from every job
  except `release.yml`'s; `Code Coverage`'s `--all-features` also needs
  `libhivex-dev`; the musl release build tried to link glibc's
  `libsystemd` into a musl target instead of building
  `--no-default-features`. Loop/NBD device nodes were root:disk 0660 —
  unreadable by the unprivileged test process, which read that as "not
  ready" instead of a permissions error; the `nbd` kernel module was
  never loaded in `ci.yml` at all. `guestkit.spec` / `guestkit-full.spec`
  were two releases stale, breaking `rpmbuild`. The k3s E2E workflow was
  missing `musl-tools` / `gcc-mingw-w64-x86-64` for the vmtools
  cross-builds, and its MinIO upload used `mc`'s default `local` alias
  credentials instead of this deployment's actual root user/password.
- **`Guestfs::launch()`** didn't transition to `Error` state when failing
  on the "no drives added" precondition — only later failures did.
- **Windows cross-compile regression** in `agent/rdp.rs` — an automated
  lint pass removed the `json` import as unused (true when checked on a
  non-Windows host) but it's real, used code behind
  `#[cfg(target_os = "windows")]`.
- **6 pre-existing doc-test compile failures** (`mem_optimize.rs`,
  `cli/parallel.rs`, `fstab_rewriter.rs`) — ambiguous generic return
  types and stale API usage in doc examples, never caught because CI
  never previously got far enough to reach the doc-test phase.

## [0.3.21] - 2026-08-08

### Fixed
- **`rescue -o reset-password` SEGV** — `hivex_value_type` FFI declaration was
  missing its two out-params (`*mut c_int`, `*mut usize`); calling it with the
  wrong signature corrupted the argument registers and crashed. Fixed the
  binding and the RID-lookup call site.
- **`ntfsfix` never actually cleared the NTFS dirty flag** — write-mode
  Windows rescue ops (`reset-password`, `enable-rdp`, ...) against a VM disk
  that wasn't cleanly shut down from inside the guest (the norm, not the
  exception, for a hypervisor-level stop) silently mounted read-only and
  no-opped until the final write, which then failed deep in the SAM upload
  step with a confusing "Read-only file system" error. `ntfsfix()` is now
  `ntfsfix_opts()` with a real `clear_dirty` flag that passes `--clear-dirty`,
  wired into all three Windows rescue-mount call sites; an interim
  `remove_hiberfile` mount-option attempt is superseded by this.
- **Windows cross-compile broken** — offline package/firstboot-stage modules
  (`cli::plan::{firstboot_stage,package_fetch,package_stage,preview}`)
  referenced Unix-only `guestfs` types with no `cfg` gate, breaking builds
  targeting `windows` for the in-guest agent. Gated with
  `#[cfg(not(target_os = "windows"))]`.

## [0.3.20] - 2026-08-07

### Added
- **DevOps runbooks** — `docs/devops/` Passport CI gate, offline repair worker,
  air-gap packages/VirtIO, fleet analyze, cutover weekend, failure triage,
  cloud disk sources (S3/GCS/Azure), forensic IR, SBOM/inventory CI.
- **GitHub Wiki** — operator cheat sheets (Passport, day-0, packages, env,
  TUI, KubeVirt/GCF/agent) linked from README / docs INDEX.
- **`GUESTKIT_PACKAGE_MIRROR`** — HTTP fallback via `curl`/`wget` when host
  `dnf`/`apt-get` is missing or fails (comma-separated bases; optional
  `{name}`/`{ext}` templates). Helps macOS hosts stage PackageInstall.
- **Domain-leave first-boot RunOnce** — `windows-domain-leave` stages
  `GuestKitDomainLeave` RunOnce (`Add-Computer -WorkGroupName`) in addition
  to Tcpip/Winlogon markers (DC computer-account delete still needs live AD).
- **Worker performance + migration profiles** — `guestkit.profile` jobs run
  the same CLI `InspectionProfile` implementations as `guestkit profile`.
- **Offline ServiceOperation / CommandExec staging** — enable/disable via
  systemd wants Symlink/FileDelete; start/restart and other commands stage
  `guestkit-firstboot-live.service` when chroot cannot run them.
- **UEFI-aware `fix-grub --force`** — detects ESP under the guest root and
  runs `grub-install --target=x86_64-efi|arm64-efi --efi-directory=…
  --no-nvram --removable` (BIOS path unchanged).
- **Windows AES/RC4 SAM NT-hash write** — `rescue -o reset-password --password`
  reconstructs the SYSKEY bootkey from SYSTEM LSA class names, derives the
  hashed bootkey from SAM `F`, and writes an AES-128-CBC (or legacy RC4)
  encrypted NT hash into the user `V` blob. Falls back to SAM blank + RunOnce
  `net user` if SYSTEM/bootkey/crypto fails.
- **PackageInstall host fetch** — with `GUESTKIT_PACKAGE_FETCH=1`, offline
  `plan apply` downloads missing `.rpm`/`.deb` on the host (`dnf download` /
  `yumdownloader` / `apt-get download`) into `GUESTKIT_PACKAGE_CACHE` or
  `~/.cache/guestkit/packages`, then stages the first-boot oneshot as before.
- **Offline GRUB repair (`fix-grub`)** — `rescue -o fix-grub` bind-mounts
  proc/sys/dev and runs chroot `grub2-mkconfig` / `grub-mkconfig` /
  `update-grub`; `--force` also attempts `grub-install` onto the NBD device
  (BIOS) or EFI removable path when an ESP is present;
  if chroot mkconfig fails, stages `guestkit-firstboot-grub.service`.
  `--export-plan` writes the first-boot FileWrite/Symlink ops. `check-grub`
  remains diagnose-only.
- **System Reserved / ESP detection** — offline Windows evidence probes
  non-OS NTFS/FAT volumes for `bootmgr` + BCD (legacy System Reserved) or
  EFI Microsoft Boot (ESP). Surfaces on `windows.system_reserved`, promotes
  `bcd_store_found` / `bootmgr_found`, fixes `esp_present` (no longer aliased
  to bootmgr). Boot check **BOOT-014**, migration **MIG-W-011**, Passport
  flags `system_reserved_layout` + `bcd_store_found`.
- **Windows driver/hotfix migration diagnostics** — offline HotFix registry +
  `$NtUninstall*` / `$hf_mig$` / CBS.log tail; VirtIO `.sys` presence on
  `WindowsDriverEntry.sys_present`; BCD UTF-16 probe for testsigning /
  nointegritychecks. Migration **MIG-W-012** (hotfixes/servicing),
  **MIG-W-013** (VirtIO files); Passport `hotfix_count` /
  `hf_mig_present` / `driver_signature_enforcement`. Hive paths resolve via
  guestfs mount root.
- **Offline activation / ghost-NIC depth** — SOFTWARE ProductId/EditionID/
  DigitalProductId + `oeminfo.ini` → `windows.activation` (OEM/Retail/Volume);
  SYSTEM `Enum\PCI` remnant/problem NICs → `ghost_nics`; Tcpip static
  interfaces → `static_nic_configs`. Enriches **MIG-W-006/007/008**; Passport
  `activation_channel`, `ghost_nic_count`, `static_nic_count`.
- **Offline BitLocker / VSS enrichment** — `BitLockerStatus\BootStatus` (On →
  hard block), FVE/`$BitLocker`/fvevol artifacts (`offline_uncertain` warning),
  VSS+swprv services + System Volume Information inference. Fills
  `windows.bitlocker` / `windows.vss` for **MIG-W-005/009**; Passport
  `bitlocker_uncertain`.
- **Day-0 plan/rescue depth** — `windows-dhcp` / `windows-dns` / `linux-hostname`
  profiles; rescue `enable-rdp` / `enable-winrm` / `set-timezone`; Windows
  `set-hostname` applies registry day-0 plan (was Linux `/etc/hostname`).
- **Cutover Passport signed-enterprise workflows** — `passport keygen` (Ed25519
  seed + pubkey); emit `--issuer` / `--expires-hours`; verify `--trust-keys`
  allowlist + `--max-age-hours` freshness gate (signing/verify need `agent`).
- **Production Helm** — `values-prod.yaml`: PVC-backed Postgres/Redis/MinIO
  (eval still `emptyDir`); Ingress TLS + cert-manager annotations; pinned
  GHCR `v0.3.19` images; nightly image-vault backup CronJob + backup PVC.
- **Guest Control Fabric poll telemetry** — airgap reconciler records per-method
  latency + transport attempts; Redis fleet rollup; `GET .../guest/poll-telemetry`
  (VM + fleet); `guest/status` exposes `lastPoll` / `telemetryMode`.
- **Fleet analyze performance** — parallel `--jobs` / `GUESTKIT_FLEET_JOBS`
  (default min(4, CPUs)); evidence-cache hit skips remount.
- **Cloud disk source depth** — persistent `~/.cache/guestkit/cloud` pulls;
  S3 `GUESTKIT_S3_ENDPOINT`/`AWS_ENDPOINT_URL`; `azure://` URIs; GCS
  `gcloud storage` fallback; CI recipe `scripts/ci-cloud-disk-sources.sh`.
- **Offline heuristic remediations + linux-grub** — `systemctl enable/disable`
  → Symlink/FileDelete; fail2ban/auditd/chrony/apparmor/sshd enable offline;
  ufw default deny FileEdit; day-0 `linux-grub` (`--grub-timeout` /
  `--grub-cmdline`) for `/etc/default/grub`.
- **Offline PackageInstall staging** — when `GUESTKIT_PACKAGE_CACHE` (or
  `host_cache`) holds matching `.rpm`/`.deb`, offline `plan apply` stages
  packages + a first-boot systemd oneshot instead of skipping; optional
  `GUESTKIT_PACKAGE_FETCH=1` downloads missing packages on the host first;
  live install unchanged.
- **Windows offline password set** — `rescue -o reset-password --password`
  prefers AES/RC4 SAM NT-hash write via SYSKEY; falls back to SAM blank +
  HKLM RunOnce `net user` for first boot; omit `--password` to blank only.

### Changed
- **Docs** — Roadmap parked list cleared; CLI / quick-reference / feature guide /
  fix-plans updated for AES SAM passwords, `fix-grub`, and
  `GUESTKIT_PACKAGE_FETCH` offline staging.

## [0.3.19] - 2026-08-06

### Added
- **Cutover Passport** — `guestkit passport emit|verify`: versioned CI-gateable
  assurance artifact (evidence digest, boot/migration scores, FixPlan digest,
  Windows BitLocker hard-block + `windows_offline_ready`, optional live
  attestation via agent-proxy, optional Ed25519 sign). Suite handoff points to
  HyperSDK (export) + hyper2kvm (convert/deploy). Web: `POST /vms/:id/passport`
  + dock download. Worker op `guestkit.passport`.
- **`plan generate -p windows-domain-leave`** — offline domain→workgroup
  markers (`--workgroup`, default `WORKGROUP`).
- **`plan generate -p windows-timezone`** — offline `TimeZoneKeyName`
  (`--timezone`).
- **`plan generate -p windows-static-ip`** — offline static IPv4 on a known
  interface GUID (`--interface-guid --ip --mask [--gateway] [--dns]`).

## [0.3.18] - 2026-08-06

### Added
- **`plan generate --profile windows-hostname`** — offline ComputerName + Tcpip
  Hostname / NV Hostname (`--hostname` required). Apply with `--skip-backup`.
- **`plan generate --profile windows-winrm`** — WinRM Automatic +
  `WINRM-HTTP-In-TCP` firewall rule. Apply with `--skip-backup`.
- **`Symlink` / `FileDelete` plan ops** — offline guestfs `ln_sf` / `rm`
  (used by hardened `linux-ssh`).
- **`plan generate -p linux-ssh --user` + `--key` / `--key-file`** — inject
  `authorized_keys` into the enable plan.
- **Windows `rescue -o reset-password`** — offline SAM blank (chntpw-style)
  via `registry-write` / libhivex; clears password so interactive logon works.
- **`rescue --export-plan PLAN.yaml`** — emit a reviewable FixPlan for
  enable-ssh / inject-ssh-key / set-hostname / reset-password / fix-fstab.
- **Offline `DriverInject`** — apply uses `host_dir` / `GUESTKIT_VIRTIO_WIN`
  + `inject_windows_driver_dir` when built with `registry-write,agent`.
- **`migrate-repair --virtio-win DIR`** — wires VirtIO host tree into
  migration repair `DriverInject` (same as `$GUESTKIT_VIRTIO_WIN`).
- **Heuristic offline remediations** — firewalld enable → `Symlink`, ufw →
  conf edit, more sshd FileEdits; preview tags live-only ops as offline-skip.

### Fixed
- **`linux-ssh` plan fidelity** — wants enable via `Symlink` (not `CommandExec
  ln`), removes `/etc/ssh/sshd_not_to_be_run`, matches rescue enable path.
- **`from_security_profile` naming** — plan profile/tags follow the inspect
  profile name (not hardcoded `"security"`).
- **`rescue check-grub`** — diagnose-only rename (`fix-grub` kept as alias).

## [0.3.17] - 2026-08-06

### Added
- **`plan generate --profile linux-ssh`** — offline Linux SSH enablement
  (systemd `ssh`/`sshd` wants symlink + `/etc/ssh/sshd_config.d/99-guestkit.conf`
  with `PubkeyAuthentication yes`). Apply with `--skip-backup`.
- **`plan` `FileWrite` operation** — create/overwrite a guest file offline.
- **`rescue inject-ssh-key`** — `--user` + `--key` / `--key-file` appends to
  `authorized_keys`.
- **`rescue set-hostname`** — `--hostname` writes `/etc/hostname` and patches
  `/etc/hosts`.

### Fixed
- **`rescue enable-ssh`** — actually creates the systemd wants symlink and
  writes an sshd drop-in (previously only printed a manual `systemctl` note);
  write drop-in before unit enable; prefer real wants dirs / relative
  symlinks / `ln -sfn` when guestfs `ln_sf` rejects unit paths.
- **Windows `guest-fsfreeze-freeze`/`thaw`** — route to VSS marker shadows
  instead of the Linux `fsfreeze` binary so KubeVirt quiesced snapshots work
  on Windows guests.

## [0.3.16] - 2026-08-06

### Added
- **`plan apply --skip-backup`** — skip the mandatory full-image qcow2/raw copy
  before apply. For low-risk registry-only plans (enable RDP, etc.) a 30–40 GiB
  Windows golden backup is slower than the edit; default remains refuse-without-backup.
- **`plan generate --profile windows-rdp`** — offline Windows Remote Desktop
  enablement plan (fDenyTSConnections, NLA, TermService/UmRdpService Automatic,
  port 3389, inbound TCP/UDP firewall Active=TRUE). Apply with `--skip-backup`.

### Fixed
- **`plan apply` dirty NTFS** — run `ntfsfix` on NTFS devices before mount
  (same path as `agent-inject --windows`) so force-off / fast-startup disks
  mount read-write and hive edits are not silently skipped (0 operations).

## [0.3.15] - 2026-07-29

### Added
- **In-guest Windows agent, fully offline install** — `guestkit agent-inject --windows`
  provisions a Windows guest with no boot required: registers the `GuestKitAgent`
  service in the `SYSTEM` hive via hivex, and installs the virtio-serial (`vioser`)
  driver the QGA channel needs (driver files, `DevicePath`, service key, and
  `CriticalDeviceDatabase` entries parsed from the INF, including the KMDF binding).
- **Stock `qemu-guest-agent` takeover** — any `QEMU-GA`/`qemu-ga`/`QEMUGuestAgent`
  service found during Windows injection is disabled (`Start=4`) so GuestKit answers
  the virtio-serial channel uncontended, while remaining QGA-compatible so
  KubeVirt/libvirt see no difference.
- **Converted-image driver fix** — deletes the stale cached
  `SYSTEM\...\Enum\PCI\VEN_1AF4&DEV_1043` device key on converted images (e.g.
  VirtualBox eval → qcow2) so the PCI bus re-detects the virtio-serial device and
  runs a full driver install on next boot instead of staying stuck on "no driver."
- **Generic `guestkit-rpc` QGA passthrough** — every in-guest agent RPC method is
  now reachable through the standard QGA channel, so host automation only needs
  `virsh qemu-agent-command`.
- **Windows agent default channel** — the Windows service now defaults to the
  virtio QGA port, matching the Linux agent's channel selection.
- Fall back to `systemctl restart` when the D-Bus `RestartUnit` call fails.

### Documentation
- Page-by-page customer manual (`docs/customer/`) with per-page PDFs, linked from
  the README.
- `docs/features/guest-agent.md` documents the Windows offline install path
  end-to-end, including the stock-QGA disable step.
- README now surfaces the in-guest agent (previously undocumented at the top
  level) with a dedicated "What's New" section, plus CI/crates.io/PyPI/license
  badges.

## [0.3.14] - 2026-07-11

### Added
- **Boot-score trend** (`guestkit-ux.js`) — every boot score is recorded per disk
  in localStorage; a re-scan after a repair toasts the delta (`▲ +N` / `▼ −N`),
  and a new **📈 Boot-score trend** command renders the history as an inline SVG
  sparkline (CSP-safe, no external assets, reduced-motion aware).
- **Zyvor brand footer + logo** — the web console and login page now carry the
  `zyvor.dev` logo (linked) and a `zyvor.dev · HyperSDK · © 2026` credit line,
  matching the PacketWolf branding treatment.

### Documentation
- **Default web console login documented** — the seeded `admin` / `Admin@321`
  (previously only printed at install time by `package-auth-bootstrap.sh`) is now
  in the remote-deploy guide, getting-started, and README, each with a
  change-on-first-login warning. Also surfaced as a first-run hint on the login
  page, shown only when local login/bypass is available.
- **Run the web stack from GHCR** — new `deploy/docker-compose.ghcr.yml` (pulls
  only the public `ghcr.io/hypersdk/{zyvor-ui,zyvor-api,guestkit-worker}` images)
  plus a "Published images (GHCR)" guide covering pull, Compose (eval), and Helm
  (prod), cross-linked from the README and deployment docs.

## [0.3.13] - 2026-07-11

### Added
- **Deep offline inspection panels** — the `guestkit.inspect` worker handler now
  collects and surfaces, from a mounted disk: partitions (device/fstype/UUID) +
  fstab, installed kernels + default, boot-load kernel modules, systemd unit
  inventory, and user accounts (`/etc/passwd` → name/uid/home/shell/login). A
  second wave adds network detail (DNS servers, default gateway), machine-id,
  cloud-init presence, VM guest tools (open-vm-tools/vmware/vbox/hyperv/qemu-ga),
  firewall (ufw/firewalld/iptables) and SSH policy (root-login/password-auth).
  The web report renders each as a token-driven panel (Storage table, Kernels,
  Drivers, Systemd units, Users table, Guest platform), all reduced-motion aware.
- **Premium web-console UX layer** (`guestkit-ux.js`) — ⌘K fuzzy command palette
  (with `>` Ask-Zeus mode), dock cursor-magnify, event-bus rich toasts, activity
  log, cinematic Zeus scan overlay + verdict burst, ambient aurora, theme wipe,
  keyboard-driven fleet nav, click-to-copy, shortcut cheat sheet, skeleton
  loaders, Ask-Zeus starter chips, global drag-to-analyze overlay, first-run
  coach-mark tour, a canvas verdict share-card (PNG export), synthesized Web
  Audio cues, a Konami "storm mode", and a client-side fleet compare view.
- **OVA + cloud-image ingest** and **multi-node CephFS RWX vault** for shared
  image storage across cluster nodes.

### Fixed
- **Windows boot doctor** — Linux-only checks (BOOT-003 Initramfs, BOOT-004 GRUB)
  are now gated as N/A on Windows guests instead of failing as false blockers.
- **Legacy-BIOS Windows BCD/bootmgr detection** — the evidence builder checked
  only EFI paths, falsely flagging `BCD store not found` on legacy installs;
  now also detects `/Boot/BCD` and `/bootmgr` at the boot-volume root.
- **Security: JWT signing key fails closed** — with `AUTH_ENABLED=true`, the API
  refuses to start unless `JWT_SECRET` is a real value (previously fell back to a
  hardcoded, globally-known `change-me-in-production` key → forgeable tokens).
- **Security: DB password out of plaintext env** — `DATABASE_URL` now comes from
  the `zyvor-secrets` Secret via `secretKeyRef` instead of being interpolated
  into the API Deployment env (visible in `kubectl describe`).
- **`delete_vm` no longer 500s on analyzed disks** — the handler tears down
  `job_results → jobs → vm_images` transactionally instead of hitting the
  `jobs_vm_id_fkey` foreign-key constraint.
- **Helm multi-tenant collision** — the KubeVirt ClusterRole/Binding names are
  namespace-scoped (`zyvor-api-kubevirt-<ns>`) so a second install doesn't clash.

## [0.3.12] - 2026-07-10

### Added
- **Offline Windows registry writes** — `registry-write` feature links libhivex (hand-rolled FFI, LGPL-2.1 dynamic link — no copyleft crate) so fix-plan `RegistryEdit` operations mutate offline SOFTWARE/SYSTEM/SAM/SECURITY hives (`HKLM`) instead of being skipped; supports REG_SZ/EXPAND_SZ/DWORD/QWORD/MULTI_SZ/BINARY with whole-disk backup. Build with `--features registry-write` (needs `libhivex-dev`/`hivex-devel`)

## [0.3.11] - 2026-06-15

### Added
- **Guest Control Fabric** — transport-independent guest control with 7-tier ladder (virtio-serial → QGA exec → QGA builtin → push cache → offline disk)
- **New API routes** — `guest/status`, `guest/capabilities`, `guest/doctor`, `guest/readiness`, `guest/install-agent`, `guest/repair-plan`, `guest/file/read|write`, `guest/poll-reconcile`
- **QGA file bootstrap** — airgap agent install via `guest-file-write` + `guest-exec` (no guest network)
- **Agent Doctor** — probe tree, readiness score (0–100), live `guestkit.doctor` via transport ladder
- **Host-mediated polling** — background reconciler for `airgap_live` VMs without push telemetry
- **GuestActionPolicy extensions** — `execAllowlist`, `fileReadAllowlist`, `fileWriteAllowlist`, `freezeAllowed`, `maxExecOutputBytes`
- **UI** — Guest Control panel, Agent Doctor tree, control-state chips, host-mediated exec warning banner
- **Docs** — [guest-control-fabric.md](../features/guest-control-fabric.md)

### Changed
- **Guest intel routes** — `/guest/*` intel endpoints return `GuestControlEnvelope` with legacy fields in `data`
- **Exec policy** — when `GuestActionPolicy` exists, `execAllowlist` is required (no raw shell by default)
- **Repair worker** — honors `inject_qga`, `fix_cloud_init_network`, `validate_fstab`, `enable_systemd` job payload fields
- **Transport ladder** — attempts VirtioSerial (daemon + socket) and InGuestSocket before QGA exec RPC
- **Offline inject** — agent binary path aligned to `/usr/local/bin/zyvor-guest-agent` and `zyvor-guest-agent` systemd unit
- **Worker repair** — honors `inject_zyvor_agent` from job payload

## [0.3.10] - 2026-06-14

### Added
- **`deploy/scripts/e2e-ubuntu-k3s.sh`** — Ubuntu 22.04 k3s E2E: offline inspect/doctor, CDI VM, live guest intel, cluster offline inspect

### Fixed
- **Release CI** — optional `journal-native` feature for musl/static builds; install `libsystemd-dev` for gnu tarballs; fix `ApiError` mapping in guest pull; align VM tools `.sha256` artifact name
- **KubeVirt QGA transport** — virt-launcher pod lookup uses `kubevirt.io/vm` (KubeVirt 1.8 labels) with fallbacks
- **Guest agent install** — KubeVirt 1.8+ virtio guestagent disk (`serial: org.qemu.guest_agent.0`) instead of rejected `devices.channels`
- **Per-VM guest pull** — install/RPC paths use `/usr/local/bin/zyvor-guest-agent`; QGA failures no longer silently fall back to in-cluster `AGENT_PROXY_URL`

## [0.3.9] - 2026-06-13

### Fixed
- `cargo fmt` formatting in security profile score calculation
- Integration tests treat `unknown` OS distro as undetected and fall back to `/etc/debian_version`
- RPM workflow installs binary package when optional Python RPM is absent; verify step uses `command -v`
- Python wheel CI installs locally built artifacts instead of stale PyPI releases
- PyPI publish uses wheel-only upload with version synced from `Cargo.toml`

## [0.3.8] - 2026-06-12

### Fixed
- CI clippy warnings across agent, AI, guestfs, and assurance modules
- Integration test uses disk-to-disk `guestkit copy` with four arguments
- `Cargo.lock` synced for RPM `--locked` builds

## [0.3.7] - 2026-06-12

### Added
- **Abyss web console** — GuestKit deploy UI with deep-navy design system (aurora background, indigo/violet accents, frosted-glass cards), local Inter fonts, and GuestKit-branded brain/dock/mission rail
- **Console Copilot API** — briefing, ask, fleet overview, compare narrative, launch advice, and system status endpoints in `zyvor-api`
- **`zyvor-guest-agent` crate** — in-guest agent daemon for Windows and Linux VM Tools
- **Windows forensic depth** — EVTX parsing, persistence run keys, forensic profile merge in evidence collectors
- **QGA helpers** — KubeVirt guest-agent transport improvements for live inspection

### Changed
- Renamed Abyss UI modules to `guestkit-console.js`, `guestkit-ai.js`, `guestkit-features.js`
- Integration tests use `guestkit copy` (replacing removed `cp` alias)
- TUI view registry includes Assurance, SystemdDeep, and AiInsights (21 views)

### Fixed
- AI agent tool-call parser accepts JSON embedded in prose lines
- Failed-disk UX in web console (deduped job tracker, disk switch guidance)
- RPM spec `%changelog` weekday dates for Fedora builds

## [0.3.6] - 2026-05-27

### Added
- **In-guest agent** — optional `agent` feature: `guestkit agent` (virtio-serial JSON-RPC daemon), `guestkit agent-proxy` (host HTTP bridge), live evidence + fix-plan execution inside running VMs
- **`guestkit-agent-protocol`** — shared length-prefixed JSON-RPC types for agent and proxy
- **`repair --inject-agent` / `migrate-plan --export --inject-agent`** — offline guestfs injection of agent binary + systemd unit
- **Worker jobs** — `guestkit.agent.evidence` and `guestkit.agent.fix` via agent-proxy HTTP
- **TUI LIVE badge** — assurance view when `GUESTKIT_AGENT_SOCKET` responds to ping
- **CI** — `agent-release.yml` musl artifact workflow; integration tests behind `--features agent`
- **TUI fix-plan preview** — read-only modal of migration plan operations (`p` in Assurance, `: plan preview`)
- **TUI Assurance shortcuts** — dashboard `a` opens Assurance; global search indexes boot blockers and migration items
- **TUI Assurance view** — Security-group panel for `doctor` boot gate and `migrate-plan` scoring; `d`/`t`/`e` keys; palette commands `doctor`, `migrate-plan`, `export plan`
- **TUI config** — `[behavior]` `default_migration_target`, `assurance_on_startup`, `show_assurance_hint`
- **TUI UX** — scrollable view tab row (`,` / `.`); compact density on Issues list rows; palette `goto` aliases for all views

### Changed
- **Dashboard** — boot score line when assurance data is loaded
- **Documentation** — pruned CLI guide and CHANGELOG; TUI assurance docs updated

## [0.3.5] - 2026-05-26

### Added
- **Migration assurance platform** — evidence snapshot model (`EvidenceSnapshot`) as the digital twin primitive for scoring engines
- **`guestkit doctor`** — bootability prediction with weighted score, blockers, and `--explain` root-cause analysis
- **`guestkit migrate-plan`** — hypervisor-aware migration scoring (KVM/Proxmox/cloud) with driver injections and downtime estimate
- **`guestkit migrate-plan --export`** — write migration guidance as an executable fix plan (YAML/JSON)
- **`guestkit policy check`** — policy-as-code alias with expression DSL over evidence fields (`bootability.score >= 80`)
- **`guestkit fleet analyze`** — cluster identical VMs, detect snowflakes, flag migration blockers
- **`guestkit forensic-diff`** — security drift scoring between two disk snapshots
- **`guestkit repair --fix boot`** — transactional boot repair via fix plans with post-apply doctor validation
- **`--profile windows-migration`** — BitLocker, domain join, RDP, hypervisor remnants, driver gaps
- **Windows registry depth** — SAM/SECURITY hive parsing, BitLocker detection, pending reboot, domain/RDP audit
- **OSV CVE lookup** — offline cache at `~/.cache/guestkit/cve/` with static fallback database
- **Cloud disk sources** — optional S3/Azure/GCS backends (`--features cloud-s3`, etc.)
- **AI evidence tools** — deterministic bootability and evidence snapshot for the optional AI assistant
- **Assurance integration tests** — CLI and plan-generation coverage for doctor/migrate-plan/repair workflow
- **Documentation** — [migration-assurance.md](../features/migration-assurance.md); VM migration and fix-plans guides updated

### Changed
- **TUI navigation** — two-tier tabs (group + view rows); scrollable jump menu and help; `{`/`}` switch groups ([tui-enhancements.md](../features/tui-enhancements.md))

## [0.3.4] - 2026-05-25

### Added
- **`guestctl` binary** — separate crate binary (alias entry point); install via `cargo install guestkit` or client tarball symlink
- **GitHub Release customer bundles** — full install tarball (`guestkit-<version>-linux-amd64.tar.gz`) matching remote deploy packaging
- **`scripts/package-binary-release.sh`** — local/CI packaging shared with GitHub Actions
- **TUI visual polish** — shared `widgets.rs` (stat chips, severity rail, progress bar, risk donut)
- **Theme variants** — `high-contrast` and `minimal` via `[ui] theme` in `tui.toml`
- **Config** — `show_emoji`, `density` under `[ui]`

### Changed
- CLI entry split into `guestkit::cli` module tree (`entry`, `invocation`, `commands_list`, `welcome`)
- TUI header, stats bar, tabs, footer, loading bar, fleet sidebar, and modal dim layer
- Dashboard and Issues views use carbon gauges, sparklines, and risk summary donut
- GitHub release workflow uploads customer bundles (gnu + musl) instead of bare binaries
- Documentation: [tui-enhancements.md](../features/tui-enhancements.md) updated for carbon theme and visual polish

## [0.3.3] - 2026-05-22

### Added
- **Carbon control-plane TUI theme** — graphite surfaces (`#0B0E12`), orange accent (`#FF7A00`) on focus and risk states only
- **Zyvor branding** on TUI splash (`zyvor.dev` wordmark)
- **Risk-aware header border** — subtle red/amber glow from security issue counts
- **Documentation hub** (`docs/INDEX.md`) with pruned user-facing docs

### Changed
- TUI footer uses muted key hints; orange reserved for primary actions
- Default TUI theme config: `carbon`
- README: open-source branding (removed Community Edition wording)

### Fixed
- TUI dashboard and issues views use consistent `content_block` pane styling

## [0.3.1] - 2026-01-26

### Added
- Killer summary view on inspect; Windows registry-based version detection
- Universal fstab/crypttab rewriter for VM migration; loop-device primary path for RAW/IMG/ISO
- LVM volume group cleanup on shutdown

## [0.3.0] and earlier

Release notes for v0.3.0 (interactive REPL, expanded inspect), v0.2.0 (extended guestfs API coverage), and v0.1.0 (initial toolkit) are in [GitHub Releases](https://github.com/zyvorai/guestkit/releases) and git history.
