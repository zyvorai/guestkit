# Zyvor VM Services on Kubernetes

Deploy GuestKit workers, zyvor-api, and supporting services for offline VM intelligence and KubeVirt migration planning.

## Components

| Service | Description |
|---------|-------------|
| `zyvor-api` | REST gateway — import, inspect, doctor, migration-plan, KubeVirt boot inspect, provision |
| `guestkit-worker` | Redis Streams worker running GuestKit operations |
| `postgresql` | VM inventory and job metadata |
| `redis` | Job queue (`zyvor:jobs` stream) |
| `minio` | VM image object storage (optional; PVC path also supported) |
| `zyvor-ui` | Minimal web console |

## Quick start

```bash
# Prerequisites: kind, kubectl, helm, docker
./deploy/scripts/kind-kubevirt-quickstart.sh
```

## Helm install

```bash
helm install zyvor deploy/helm/zyvor -n zyvor --create-namespace
```

## k3s deploy (Ubuntu)

Single-node k3s on Ubuntu (remote host or CI):

```bash
# Installs k3s, helm, qemu-utils, podman; deploys the full stack
bash deploy/scripts/install-k3s-ubuntu.sh
bash deploy/scripts/deploy-remote-k3s.sh

# CI overlay (no hardcoded host IP — URLs computed at deploy time)
HELM_VALUES_FILE=values-ci.yaml bash deploy/scripts/deploy-remote-k3s.sh

# Pull release images from GHCR instead of local build
PULL_REGISTRY=ghcr.io/zyvorai IMAGE_TAG=v1.0.1 bash deploy/scripts/deploy-remote-k3s.sh
```

Override public URLs when the node IP is not the client-facing address:

```bash
NODE_IP=203.0.113.10 \
ZEUS_PUBLIC_URL=http://203.0.113.10:30080 \
VMTOOLS_BASE_URL=http://203.0.113.10:30092/vmtools \
bash deploy/scripts/deploy-remote-k3s.sh
```

Helm overlays:

| File | Use |
|------|-----|
| `values-k3s.yaml` | Remote k3s (URLs via env/`--set`) |
| `values-ci.yaml` | GitHub Actions `k3s-e2e.yml` |

## Fresh-cluster install notes

A bare `helm install zyvor deploy/helm/zyvor -n zyvor --create-namespace` will
**not** come up as-is on a stock cluster. The first-party images default to
unregistered `:latest`, and the default StorageClass (`longhorn`) usually does
not exist. A working from-scratch install (verified) overrides those:

```bash
helm install zyvor deploy/helm/zyvor -n zyvor --create-namespace \
  --set namespace=zyvor \
  --set zyvorApi.image=ghcr.io/zyvorai/zyvor-api:<ver> \
  --set zyvorUi.image=ghcr.io/zyvorai/zyvor-ui:<ver> \
  --set guestkitWorker.image=ghcr.io/zyvorai/guestkit-worker:<ver> \
  --set zyvorApi.storageClass=<sc> \
  --set persistence.vmImages.storageClass=<sc> \
  --set persistence.vmImages.accessMode=ReadWriteMany   # RWX for multi-node; RWO single-node
```

Notes:
- `.Values.namespace` is independent of `.Release.Namespace` — set both.
- KubeVirt + CDI, an Ingress controller, and (for the k3s overlay) a CephFS/RWX
  provider are **prerequisites**, not bundled.
- The KubeVirt ClusterRole/Binding are namespace-scoped
  (`zyvor-api-kubevirt-<ns>`), so multiple installs coexist.

## Production hardening (before real customer data)

These are deployment-time decisions the chart leaves to the operator:

- **Auth**: off by default. Enable with `--set zyvorApi.auth.enabled=true` and a
  real `--set zyvorApi.auth.jwtSecret=$(openssl rand -base64 32)`. The API
  **fails closed** — it refuses to start if auth is on without a real secret.
- **Secrets**: change the default `postgresql.password` / `minio` keys. The DB
  DSN is delivered via the `zyvor-secrets` Secret (not plaintext env).
- **Persistence**: postgres and minio use `emptyDir` by default → **data is lost
  on pod restart**. Wire PVCs before production.
- **TLS**: the Ingress is HTTP-only (`ssl-redirect: false`); terminate TLS at
  your ingress / LB (e.g. cert-manager). Agent↔API is already mTLS on 8443.
- **Backup**: no built-in backup/restore for the DB or image vault — add your own
  (`pg_dump` CronJob, vault snapshot).

## Release artifacts (`v*` tags)

The [release workflow](../.github/workflows/release.yml) publishes:

| Asset | Destination |
|-------|-------------|
| `guestkit-<ver>-linux-amd64(.tar.gz)` | GitHub Release |
| `zyvor-vm-tools-linux-amd64.tar.gz`, `.deb`, optional `.iso` | GitHub Release |
| `ghcr.io/zyvorai/guestkit-worker:v<ver>` | GHCR |
| `ghcr.io/zyvorai/zyvor-api:v<ver>` | GHCR |
| `ghcr.io/zyvorai/zyvor-ui:v<ver>` | GHCR |

Build images locally with `deploy/scripts/publish-images.sh`. The root `Dockerfile` is deprecated; use per-service Dockerfiles under `crates/` and `deploy/ui/`.

Manual agent-only rebuild: [agent-release.yml](../.github/workflows/agent-release.yml) (`workflow_dispatch`).

## CI k3s E2E

[`.github/workflows/k3s-e2e.yml`](../.github/workflows/k3s-e2e.yml) on `ubuntu-latest`:

1. `install-k3s-ubuntu.sh` — k3s, helm, qemu-utils
2. `deploy-remote-k3s.sh` — PRs build images locally; `v*` tags pull GHCR release images
3. `publish-vmtools-bundle.sh` — agent binaries into in-cluster MinIO
4. `ci-k3s-e2e.sh` — API health, `e2e-smoke.sh` (cirros), VM tools bundle/API checks

**Ubuntu live E2E** (manual, on a k3s host with MinIO + KubeVirt):

```bash
API=http://<node-ip>:30080/api/v1 \
AGENT_NODE=http://<node-ip>:30092 \
bash deploy/scripts/e2e-ubuntu-k3s.sh
```

Imports Ubuntu 22.04, runs offline inspect/doctor, provisions a CDI VM with IP-only cloud-init (QGA deb + `zyvor-guest-agent` from MinIO), exercises **Guest Control Fabric** routes (`guest/status`, `guest/doctor`, airgap `guest/install-agent`), live guest intel, then offline cluster inspect/doctor on a halted VM.

**Airgap install:** when QGA is up but guest network is down, `POST .../guest/install-agent` with `strategy: auto` selects QGA file bootstrap (chunked `guest-file-write`, no curl in guest). See [guest-control-fabric.md](../docs/features/guest-control-fabric.md).

Trigger manually:

```bash
gh workflow run k3s-e2e.yml
```

## API

See [openapi/zyvor-vm-services.yaml](openapi/zyvor-vm-services.yaml).

Example workflow:

```bash
API=http://api.zyvor.local/api/v1

# Import
curl -F "file=@disk.vmdk" $API/vms/import

# Doctor
curl -X POST "$API/vms/{id}/doctor?target=kubevirt&explain=true"

# Migration plan
curl -X POST "$API/vms/{id}/migration-plan?target=kubevirt"

# Provision KubeVirt YAML
curl -X POST "$API/vms/{id}/provision"

# Offline boot inspect (stopped KubeVirt VM — pure Rust GuestKit, not legacy appliance tooling)
curl "$API/kubevirt/vms/default/my-vm/boot-inspect"
curl -X POST "$API/kubevirt/boot-inspect" \
  -H 'Content-Type: application/json' \
  -d '{"namespace":"default","vm":"my-vm","mode":"boot-inspect","source":"zeus-os"}'
```

See [docs/features/kubevirt-integration.md](../docs/features/kubevirt-integration.md) for Zeus OS Guest Intelligence integration.

## KubeVirt

KubeVirt + CDI are **cluster prerequisites**. The Helm chart generates VM/DataVolume YAML; it does not install KubeVirt.

See [examples/kubevirt/migrated-vm.yaml](examples/kubevirt/migrated-vm.yaml).

## Legacy worker DaemonSet

The file-based worker in [`k8s/`](../k8s/) remains available for development. Production deployments should use the Helm chart with Redis transport.
