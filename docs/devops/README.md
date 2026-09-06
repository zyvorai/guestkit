# DevOps runbooks — GuestKit

Operational docs for platform / migration / SRE teams who gate cutovers with **GuestKit** (doctor, migrate-plan, rescue, Passport) before **hyper2kvm** / HyperSDK convert.

| Runbook | When you need it |
|---------|------------------|
| [01 — Passport CI gate](01-passport-ci-gate.md) | Fail convert if score &lt; floor — GitHub Actions: [`zyvorai/guestkit` action](01-passport-ci-gate.md#1-github-actions--zyvoraiguestkit-composite-action) |
| [02 — Offline repair worker](02-offline-repair-worker.md) | Jump box / GHCR worker, root, NBD |
| [03 — Air-gap packages & VirtIO](03-airgap-packages-virtio.md) | Mirror, cache, `GUESTKIT_VIRTIO_WIN` |
| [04 — Fleet analyze at scale](04-fleet-analyze.md) | Directory of images, snowflakes |
| [05 — Cutover weekend](05-cutover-weekend.md) | Hour-by-hour ops checklist |
| [06 — Failure triage](06-failure-triage.md) | Doctor red, BitLocker, GRUB, SAM |
| [07 — Cloud disk sources](07-cloud-disk-sources.md) | s3:// gs:// azure:// pulls + cache |
| [08 — Forensic diff & IR](08-forensic-diff-ir.md) | Offline drift / secrets / malware |
| [09 — SBOM / inventory CI](09-sbom-inventory-ci.md) | SPDX/CycloneDX artifacts |
| [10 — Rescue dry-run + SBOM diff](10-rescue-sbom-ci.md) | Action extras, `sbom-diff --fail-on-drift` |

**Related:** [Wiki](https://github.com/zyvorai/guestkit/wiki) · [Migration assurance](../features/migration-assurance.md) · [DOCKER / GHCR](../guides/DOCKER.md) · Blogs: [integrate pipeline](https://zyvor.dev/blog/guestkit-integrate-migration-pipeline) · [DevOps runbooks](https://zyvor.dev/blog/guestkit-devops-runbooks)

## Operating model

| Role | Owns |
|------|------|
| **Migration / DevOps** | Worker image, CI job, `--fail-below`, signing keys, mirror URL, change-ticket Passport attach |
| **Platform** | Disk staging (object store / NFS), runner privileges (`qemu-nbd`, loop), network to mirrors |
| **App owners** | Accept day-0 plans (RDP, SSH keys, domain leave), rotate temp passwords |
| **Convert tool** | hyper2kvm / suite — only after `passport verify` green |

## Pin

```text
CLI:     cargo install guestkit   # or distro/package pin
Worker:  ghcr.io/zyvorai/guestkit-worker   # pin digest/tag in compose/Helm
```
