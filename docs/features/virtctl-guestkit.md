# Replace virtctl guestfs with GuestKit

`virtctl guestfs` starts a libguestfs-tools pod and attaches a shell. GuestKit
owns that workflow now via the existing `virtctl-guestkit` / `kubectl-guestkit`
plugin (same package, no extra crate).

Lifecycle stays on upstream virtctl: `start`, `stop`, `migrate`, `console`,
`vnc`, `ssh`.

## Install

```bash
cargo build --release --bin virtctl-guestkit
sudo install -m 0755 target/release/virtctl-guestkit /usr/local/bin/virtctl-guestkit
sudo ln -sf virtctl-guestkit /usr/local/bin/kubectl-guestkit
```

## Commands

```bash
# drop-in for: virtctl guestfs -n ns pvc
virtctl-guestkit guestfs -n ns pvc
kubectl guestkit guestfs -n ns pvc
virtctl-guestkit guestfs --vm my-vm -n ns

virtctl-guestkit inspect --vm my-vm -n ns
virtctl-guestkit doctor  --vm my-vm -n ns
virtctl-guestkit rescue  -n ns pvc -o enable-ssh

# local disk (unchanged)
virtctl-guestkit doctor --image disk.qcow2 --target kubevirt
```

Filesystem PVCs mount at `/disk`. Block PVCs attach as `/dev/vda`. The pod is
deleted when the session ends. The VM must be stopped so the PVC is free.

Requires `kubectl` on PATH (`$KUBECTL` override). Image:
`$GUESTKIT_IMAGE` or `ghcr.io/zyvorai/guestkit:latest`.
