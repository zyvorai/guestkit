# Passport handoff, fleet quarantine, virtctl plugin

## `guestkit passport handoff`

```bash
guestkit passport emit disk.qcow2 --target kvm -o passport.json
guestkit passport handoff passport.json                 # writes passport.handoff.yaml
guestkit passport handoff passport.json -o job.yaml --fail-below 80
guestkit passport handoff passport.json --allow-refused # write YAML even when blocked
```

The document:

```yaml
apiVersion: guestkit.zyvor.dev/v1
kind: H2kvmHandoff
passport: passport.json
image: /var/lib/libvirt/images/web.qcow2
target: kvm
scores: { boot: 91.0, migration: 88.0 }
hard_blocked: false
allowed: true
h2kvmctl: h2kvmctl local --to-output out.qcow2 --backend guestkit --passport passport.json
```

`allowed: false` means **do not convert**.

## `guestkit fleet quarantine`

```bash
guestkit fleet quarantine ./images --threshold 80 --fail -o json
```

Reasons: `low_score`, `analyzer_blocker` (score < 60 from fleet analyze),
`collect_failed`.

## virtctl / kubectl plugin

```bash
cargo build --release --bin virtctl-guestkit
sudo install -m 0755 target/release/virtctl-guestkit /usr/local/bin/virtctl-guestkit
sudo ln -sf virtctl-guestkit /usr/local/bin/kubectl-guestkit

virtctl guestkit doctor --image disk.qcow2 --target kubevirt
kubectl guestkit handoff --passport passport.json

# replaces `virtctl guestfs` (VM must be stopped)
virtctl-guestkit guestfs -n default disk-pvc
virtctl-guestkit guestfs --vm my-vm -n default
virtctl-guestkit inspect --vm my-vm -n default
virtctl-guestkit doctor --vm my-vm -n default
```

Local `--image` still works without a cluster. PVC attach uses `kubectl` and
the GuestKit image (`$GUESTKIT_IMAGE` or `ghcr.io/zyvorai/guestkit:latest`).
