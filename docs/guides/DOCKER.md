# Docker Deployment Guide

This guide covers running guestkit in containers — both the **web stack from
published GHCR images** (no build required) and the **single-container CLI** for
automation and batch processing.

- **Run the web console from GHCR** → [Published images](#published-images-ghcr) (pull + `docker compose up`)
- **Run the CLI in a container** → [Quick Start](#quick-start) (build locally)

---

## Published images (GHCR)

The web stack is published to the GitHub Container Registry under
**`ghcr.io/hypersdk`**. The packages are **public — no `docker login` needed to pull.**

| Image | Role | Port |
|-------|------|------|
| `ghcr.io/hypersdk/zyvor-ui` | Web console + login page (nginx) | 80 |
| `ghcr.io/hypersdk/zyvor-api` | API backend (auto-runs DB migrations) | 8080 |
| `ghcr.io/hypersdk/guestkit-worker` | Disk-inspection worker (Redis queue) | — |

**Tags:** `latest`, semver `vX.Y.Z` (e.g. `v1.0.1`), and a per-commit short SHA.
Published automatically by CI — `publish-zyvor-images.yml` on every push to `main`
(tags `:<sha>` + `:latest`) and `release.yml` on a release (tags `:vX.Y.Z` + `:latest`).

### Pull

```bash
docker pull ghcr.io/hypersdk/zyvor-ui:latest
docker pull ghcr.io/hypersdk/zyvor-api:latest
docker pull ghcr.io/hypersdk/guestkit-worker:latest
```

### Run the full stack (evaluation)

> **Evaluation only — not for production.** The eval compose file (`deploy/docker-compose.ghcr.yml`)
> runs with `AUTH_ENABLED=false`, no Redis password, and no agent bootstrap token. Use it on
> **localhost only**. For production, see [Production checklist](#production-checklist) and
> `deploy/docker-compose.prod.example.yml`.

The stack needs Postgres + Redis behind the three images. A ready-to-run compose
file ships in the repo — it pulls only from GHCR (nothing is built locally):

```bash
# From the repo root
docker compose -f deploy/docker-compose.ghcr.yml pull
docker compose -f deploy/docker-compose.ghcr.yml up -d

# Open the web console
open http://localhost:8088          # macOS  (Linux: xdg-open)
```

The console Image Vault supports inspect inventory tabs, Assurance (doctor / plan / passport / repair), Profiles, and Files browse when backed by a current zyvor-api + guestkit-worker. See [Using the Dashboard](../customer/using-the-dashboard.md).

Pin a version or a different registry with env vars:

```bash
REGISTRY=ghcr.io/hypersdk TAG=v1.0.1 \
  docker compose -f deploy/docker-compose.ghcr.yml up -d
```

The eval stack starts with `AUTH_ENABLED=false`, so the console opens without a
login. To require sign-in, set `AUTH_ENABLED=true` and a strong `JWT_SECRET` on
the `zyvor-api` service — see
[Web console access](DEPLOY-REMOTE.md#web-console-access) for the default
`admin` / `Admin@321` credentials and how to change them.

> The `zyvor-ui` container's nginx proxies `/api/` to `http://zyvor-api:8080`, so
> the API service must keep the name **`zyvor-api`** if you write your own compose.

### Run the full stack (production) — Helm

For clusters, use the Helm chart in [`deploy/helm/zyvor`](../../deploy/helm/zyvor),
pointing each image at GHCR:

```bash
helm upgrade --install zyvor deploy/helm/zyvor \
  --create-namespace --namespace zyvor \
  --set guestkitWorker.image=ghcr.io/hypersdk/guestkit-worker:v1.0.1 \
  --set zyvorApi.image=ghcr.io/hypersdk/zyvor-api:v1.0.1 \
  --set zyvorUi.image=ghcr.io/hypersdk/zyvor-ui:v1.0.1
```

The chart also provisions Postgres, Redis, and MinIO. Enable auth via
`--set zyvorApi.auth.enabled=true` and supply `jwtSecret` and `agentBootstrapToken`
through a secret. Use `deploy/helm/zyvor/values-prod.yaml` as a starting point
(PVC datastores, TLS/cert-manager, pinned tags, nightly image-vault backup).

### Production checklist

Before exposing the web stack beyond localhost, verify:

| Item | Eval default | Production requirement |
|------|--------------|------------------------|
| Authentication | `AUTH_ENABLED=false` | `AUTH_ENABLED=true` + strong `JWT_SECRET` |
| Agent bootstrap | Open registration | `AGENT_BOOTSTRAP_TOKEN` set (required when auth or mTLS is on) |
| Redis | No password | `REDIS_PASSWORD` / `redis.password` in Helm |
| Postgres | `zyvor`/`zyvor` | Strong unique password |
| Image tags | `:latest` | Pin semver (e.g. `v1.0.1`) |
| Datastores | `emptyDir` | Enable `persistence.{postgresql,redis,minio}` PVCs via `values-prod.yaml` |
| Ingress TLS | Off / `ssl-redirect: false` | `ingress.tls` + cert-manager ClusterIssuer |
| Image vault backup | Off | `backup.imageVault.enabled` CronJob → backup PVC |
| Worker | `privileged: true` | Isolate on dedicated nodes; network-policy Redis |
| Exposure | localhost:8088 | TLS ingress; do not publish eval compose to the internet |

**Generate secrets:**

```bash
openssl rand -base64 32   # JWT_SECRET
openssl rand -base64 32   # AGENT_BOOTSTRAP_TOKEN
openssl rand -base64 24   # POSTGRES_PASSWORD / REDIS_PASSWORD
```

**Docker Compose (production example):**

```bash
cp deploy/docker-compose.prod.example.yml deploy/docker-compose.prod.yml
# Edit deploy/docker-compose.prod.yml — set TAG, JWT_SECRET, AGENT_BOOTSTRAP_TOKEN, passwords
docker compose -f deploy/docker-compose.prod.yml up -d
```

**Helm (production):**

```bash
helm upgrade --install zyvor deploy/helm/zyvor \
  -f deploy/helm/zyvor/values.yaml \
  -f deploy/helm/zyvor/values-prod.yaml \
  --create-namespace --namespace zyvor
```

The API **refuses to start** when `AUTH_ENABLED=true` or `AGENT_MTLS_BIND_ADDR` is set
without `AGENT_BOOTSTRAP_TOKEN`. Startup logs warn when auth is disabled or the bootstrap
token is unset.

---

## Quick Start

### Build the Image

```bash
docker build -t guestkit:latest .
```

### Run a Simple Inspection

```bash
docker run --privileged \
  -v /path/to/vms:/vms:ro \
  -v $(pwd)/output:/output \
  guestkit:latest inspect /vms/vm.qcow2
```

### Using Docker Compose

```bash
# Create a vms directory with your VM images
mkdir -p vms output

# Run with docker-compose
docker-compose run guestkit inspect /vms/vm.qcow2 --output json > output/report.json
```

## Why Privileged Mode?

Guestkit requires privileged access because it:

1. **Loads kernel modules** - `nbd` and `loop` drivers
2. **Creates device nodes** - `/dev/nbd*` and `/dev/loop*`
3. **Mounts filesystems** - Inspects disk partitions without booting

### Security Considerations

**Privileged mode grants extensive access.** For production use:

- Run in isolated environments
- Use read-only volume mounts for VM images
- Limit network access
- Run as non-root user where possible (see below)

### Alternative: Minimal Capabilities

Instead of `--privileged`, you can use specific capabilities:

```bash
docker run \
  --cap-add=SYS_ADMIN \
  --cap-add=MKNOD \
  --cap-add=SYS_MODULE \
  --device=/dev/nbd0 \
  --device=/dev/loop0 \
  -v /path/to/vms:/vms:ro \
  guestkit:latest inspect /vms/vm.qcow2
```

**Note:** This is more restrictive but may require pre-loaded kernel modules on the host.

## Common Use Cases

### 1. Batch Inspection with JSON Output

```bash
docker run --privileged \
  -v $(pwd)/vms:/vms:ro \
  -v $(pwd)/output:/output \
  guestkit:latest inspect-batch /vms/*.qcow2 \
    --parallel 4 \
    --output json > output/inventory.json
```

### 2. Security Profiling

```bash
docker run --privileged \
  -v $(pwd)/vms:/vms:ro \
  guestkit:latest profile security /vms/production-vm.qcow2 \
    --export /output/security-report.json
```

### 3. VM Comparison

```bash
docker run --privileged \
  -v $(pwd)/vms:/vms:ro \
  guestkit:latest diff \
    /vms/vm-before.qcow2 \
    /vms/vm-after.qcow2 \
    --output json
```

### 4. CI/CD Pipeline Integration

```yaml
# .github/workflows/vm-security-scan.yml
name: VM Security Scan

on:
  push:
    paths:
      - 'vm-images/**'

jobs:
  security-scan:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v3

      - name: Build guestkit
        run: docker build -t guestkit:ci .

      - name: Run security profile
        run: |
          docker run --privileged \
            -v ${{ github.workspace }}/vm-images:/vms:ro \
            guestkit:ci profile security /vms/*.qcow2 \
              --output json > security-report.json

      - name: Upload report
        uses: actions/upload-artifact@v3
        with:
          name: security-report
          path: security-report.json
```

### 5. REST API Wrapper

Create a simple API service:

```dockerfile
# Dockerfile.api
FROM guestkit:latest

RUN apt-get update && apt-get install -y python3 python3-pip
RUN pip3 install fastapi uvicorn

COPY api.py /app/api.py

EXPOSE 8000
CMD ["uvicorn", "app.api:app", "--host", "0.0.0.0", "--port", "8000"]
```

```python
# api.py
from fastapi import FastAPI, UploadFile
import subprocess
import json

app = FastAPI()

@app.post("/inspect")
async def inspect_vm(file: UploadFile):
    # Save uploaded VM image
    vm_path = f"/tmp/{file.filename}"
    with open(vm_path, "wb") as f:
        f.write(await file.read())

    # Run guestkit
    result = subprocess.run(
        ["guestkit", "inspect", vm_path, "--output", "json"],
        capture_output=True,
        text=True
    )

    return json.loads(result.stdout)
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Log level (error, warn, info, debug, trace) |
| `OPENAI_API_KEY` | - | OpenAI API key for AI diagnostics (optional) |
| `GUESTKIT_CACHE_DIR` | `/cache` | Cache directory for inspection results |
| `GUESTKIT_CONFIG_DIR` | `/config` | Configuration directory |

## Volume Mounts

| Host Path | Container Path | Purpose | Mode |
|-----------|----------------|---------|------|
| `./vms` | `/vms` | VM disk images | `ro` (read-only) |
| `./output` | `/output` | Export reports | `rw` |
| `/dev` | `/dev` | Device access | `rw` (required) |
| Named volume | `/cache` | Inspection cache | `rw` |
| Named volume | `/config` | TUI config | `rw` |

## Caching for Performance

Enable caching for repeated inspections:

```bash
docker-compose run guestkit inspect-batch /vms/*.qcow2 --cache --parallel 4
```

Cache is persisted in the `guestkit-cache` Docker volume.

## Kubernetes Deployment

For Kubernetes, you'll need a privileged pod:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: guestkit-job
spec:
  containers:
  - name: guestkit
    image: guestkit:latest
    securityContext:
      privileged: true
    volumeMounts:
    - name: vms
      mountPath: /vms
      readOnly: true
    - name: output
      mountPath: /output
    - name: dev
      mountPath: /dev
    command: ["guestkit", "inspect-batch", "/vms/*.qcow2", "--parallel", "4"]
  volumes:
  - name: vms
    hostPath:
      path: /path/to/vms
  - name: output
    emptyDir: {}
  - name: dev
    hostPath:
      path: /dev
  restartPolicy: Never
```

## Limitations

### Interactive Features Not Recommended

These features work better natively than in containers:

- **TUI Dashboard** - Terminal handling is complex in containers
- **Interactive Shell** - Better user experience natively
- **Fuzzy navigation** - Keyboard input may be problematic

### Host Kernel Dependencies

The container relies on the **host kernel** for:
- NBD module (`nbd.ko`)
- Loop device support

Ensure these are available on the host:

```bash
# On host machine
sudo modprobe nbd max_part=8
sudo modprobe loop
```

## Troubleshooting

### "Cannot load nbd module"

**Cause:** NBD module not available on host kernel

**Solution:**
```bash
# On host
sudo modprobe nbd max_part=8
```

### "Permission denied" accessing /dev

**Cause:** Container not running in privileged mode

**Solution:** Add `--privileged` flag or proper capabilities

### "No such device /dev/nbd0"

**Cause:** NBD devices not created

**Solution:**
```bash
# On host, create NBD devices
for i in {0..15}; do
  sudo mknod /dev/nbd$i b 43 $i
done
```

### Build fails with "disk quota exceeded"

**Cause:** Insufficient disk space

**Solution:**
```bash
# Clean up Docker resources
docker system prune -a
```

## Production Recommendations

1. **Use specific image tags** - Don't use `:latest` in production
2. **Enable read-only rootfs** - Add `--read-only` flag where possible
3. **Resource limits** - Set CPU and memory constraints
4. **Network isolation** - Use `--network none` if network isn't needed
5. **Security scanning** - Scan images with tools like Trivy
6. **Log aggregation** - Collect logs via Docker logging drivers

## Building with Features

### With AI Support

```dockerfile
# In Dockerfile, change the build command
RUN cargo build --release --features ai --bin guestkit
```

### With Python Bindings

```dockerfile
# In Dockerfile, change the build command
RUN cargo build --release --features python-bindings --bin guestkit
```

## Alternatives to Docker

### Podman

Guestkit works with Podman (rootless or rootful):

```bash
podman build -t guestkit:latest .
podman run --privileged -v ./vms:/vms:ro guestkit:latest inspect /vms/vm.qcow2
```

### Singularity/Apptainer

For HPC environments:

```bash
singularity build guestkit.sif docker://guestkit:latest
singularity run --writable-tmpfs guestkit.sif inspect /vms/vm.qcow2
```

## Further Reading

- [Published images (GHCR)](#published-images-ghcr) - Pull + `docker compose` / Helm for the web stack
- [Remote deploy](DEPLOY-REMOTE.md) - SSH deploy of the CLI + web console access
- [Helm chart](../../deploy/helm/zyvor) - Production Kubernetes install
- [Project README](../../README.md) - Main documentation
- [Docker Security Best Practices](https://docs.docker.com/engine/security/)
