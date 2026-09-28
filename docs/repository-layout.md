# Repository

> Part of the [GuestKit README](../README.md). Back to the [documentation map](documentation-map.md).

```text
src/                      library + CLI (guestkit, guestctl, guestkit-qemu, virtctl-guestkit)
  assurance/              doctor, migrate-plan and repair APIs
  boot/                   bootability prediction engine
  evidence/               normalized evidence snapshot (what every score is computed from)
  inference/              deterministic root-cause inference
  migration/              migration readiness assessment and repair planning
  fleet/                  fleet clustering, drift and anomaly detection
  disk/  guestfs/         pure-Rust disk, partition and filesystem handling
  converters/             disk format conversion
  storage/                local and cloud (S3 / GCS / Azure) disk sources
  qemu/                   assured QEMU / VirtIO definitions for guestkit-qemu
  agent/  collectors/     in-guest agent daemon, host proxy and live collectors
  ai/                     optional evidence-grounded analysis (--features ai)
  export/  templates/     JSON / YAML / HTML / PDF / Markdown reports
  cli/                    command implementations
crates/
  guestkit-agent-protocol/  JSON-RPC types for the in-guest agent
  guestkit-job-spec/        VM operations job protocol
  guestkit-worker/          distributed disk-inspection worker
  zyvor-api/                API gateway behind the web console
  zyvor-guest-agent/        in-guest agent (Linux + Windows)
deploy/                   web console (ui/), Helm, compose files, CRDs, OpenAPI
k8s/                      KubeVirt boot-inspect DaemonSet manifests
policies/                 Rego cutover policy
examples/                 Rust, Python and job-spec examples
tests/  integration/      unit, integration and realistic-image tests
website/                  Docusaurus docs site (serves ../docs)
docs/                     user guides, feature docs, DevOps runbooks, roadmap
docs/social/              share card source (HTML) and render script
docs/img/                 README and site screenshots
```
