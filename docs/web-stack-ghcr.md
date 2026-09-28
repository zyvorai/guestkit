# Run the free web stack (GHCR)

> Part of the [GuestKit README](../README.md). Back to the [documentation map](documentation-map.md).

Public images under **`ghcr.io/zyvorai`** — no `docker login` required.

| Image | Role |
|-------|------|
| `ghcr.io/zyvorai/zyvor-ui` | Web console — Image Vault, KubeVirt cluster |
| `ghcr.io/zyvorai/zyvor-api` | API |
| `ghcr.io/zyvorai/guestkit-worker` | Disk-inspection worker |

```bash
docker compose -f deploy/docker-compose.ghcr.yml pull
docker compose -f deploy/docker-compose.ghcr.yml up -d
open http://localhost:8088
```

> **Eval only** — unauthenticated stack. Do not expose beyond localhost.  
> Production: `deploy/docker-compose.prod.example.yml` · [Docker guide](guides/DOCKER.md) · [Helm](../deploy/helm/zyvor)
