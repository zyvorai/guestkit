# Prerequisites and build

> Part of the [GuestKit README](../README.md). Back to the [documentation map](documentation-map.md).

# Prerequisites

- **Running GuestKit:** Linux with `qemu-img`, `losetup` and `qemu-nbd`. Mount and repair may need root. The web console and worker run anywhere Docker does — see [Run the free web stack](web-stack-ghcr.md).
- **Building from source:** a stable Rust toolchain (edition 2021) and the libsystemd development headers, which the default `journal-native` feature links. Optional features (`ai`, `mcp`, `registry-write`, the Windows agent) have extra requirements — see the `Makefile` and [CONTRIBUTING](development/CONTRIBUTING.md).
- **Python bindings:** `pip install zyvor-guestkit` needs nothing else; building them yourself uses `build_python.sh`.

# Build

```bash
cargo build --release     # guestkit, guestctl, guestkit-qemu, virtctl-guestkit
cargo test
make check                # tests + clippy
make install              # installs the guestkit binary (PREFIX=$HOME/.local to change the location)
```

See [CONTRIBUTING](development/CONTRIBUTING.md) and CI under `.github/workflows/`. **`docs/` and this README are authoritative.**
