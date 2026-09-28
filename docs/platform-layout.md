# Platform layout

> Part of the [GuestKit README](../README.md). Back to the [documentation map](documentation-map.md).

```text
┌────────────────────────────────────────────────────────────┐
│  guestkit CLI · guestctl TUI · guestkit-qemu · Python · Web │
├────────────────────────────────────────────────────────────┤
│  Rust evidence · boot scoring · fix-plan · QEMU/VirtIO plan │
├────────────────────────────────────────────────────────────┤
│  JSON · YAML · HTML · PDF · Passport · CI exit codes       │
└────────────────────────────────────────────────────────────┘
```

| Layer | In this repo |
|-------|----------------|
| **Engine** | Pure-Rust parsers + evidence schema · NBD/loop (`src/`, `crates/`) |
| **CLI / TUI** | `guestkit` · `guestctl` — doctor, passport, fleet, rescue |
| **QEMU runtime** | `guestkit-qemu` — assured plan/run + QMP ([qemu-runtime.md](features/qemu-runtime.md)) |
| **Agent / QGA** | Linux + Windows · `agent-inject` / `agent-proxy` / **`guestkit qga`** ([virsh-to-guestkit.md](user-guides/virsh-to-guestkit.md)) |
| **Python** | [zyvor-guestkit](https://pypi.org/project/zyvor-guestkit/) — `run_doctor`, `run_migrate_repair` (v1.1.0+) |
| **h2kvm** | [hyper2kvm-integration.md](features/hyper2kvm-integration.md) — convert/deploy partner |
| **FluxVM** | [zyvorai/fluxvm](https://github.com/zyvorai/fluxvm) — run/manage certified qcow2s (network, TTL) |
| **K8s** | KubeVirt hooks · `k8s/` |
| **Web / worker** | GHCR images · `deploy/` |
