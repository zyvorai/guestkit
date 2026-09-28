# The cutover problem — solved offline

> Part of the [GuestKit README](../README.md). Back to the [documentation map](documentation-map.md).

# The cutover problem — solved offline

Every hypervisor exit fails the same way: you discover the disk was broken **at 2am**, in the cutover window, after power-on.

GuestKit reads the disk **while the guest is off**, scores first-boot probability 0–100, and emits a reviewable fix plan — no appliance daemon, no “just try it and hope.”

```text
  disk.qcow2 / .vmdk / .vhdx / .vhd / .vdi / .raw
                    │
                    ▼
         ┌──────────────────────┐
         │  Pure-Rust engine    │──►  doctor 0–100 + blockers
         │  NBD / loop mount    │──►  migrate-plan YAML
         └──────────────────────┘──►  Passport · repair · CI gate
                    │                 guestkit-qemu (assured launch)
      CLI · TUI · QEMU · Python · Web · Agent · GitHub Action
```

| | |
|---|---|
| **70+** commands | **6** disk formats |
| **0** appliance daemons | **8** migration targets |
| **Apache-2.0** | Used in CI, labs, and hypervisor-exit programs |

**Certify with [GuestKit](https://github.com/zyvorai/guestkit) → run & manage with [FluxVM](https://github.com/zyvorai/fluxvm) → convert & deploy with [h2kvm](https://github.com/zyvorai/h2kvm) → operate on [Zeus OS](https://zyvor.dev/zeus-os).**
