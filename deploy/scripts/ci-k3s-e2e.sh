#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# CI orchestrator: health, e2e-smoke, VM tools bundle/API checks.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
NAMESPACE="${NAMESPACE:-zyvor}"

resolve_node_ip() {
  local ip
  ip="$(kubectl get nodes -o jsonpath='{.items[0].status.addresses[?(@.type=="InternalIP")].address}' 2>/dev/null || true)"
  if [[ -z "${ip}" ]]; then
    ip="$(hostname -I 2>/dev/null | awk '{print $1}')"
  fi
  echo "${ip}"
}

NODE_IP="${NODE_IP:-$(resolve_node_ip)}"
API_PORT="${API_PORT:-30080}"
MINIO_PORT="${MINIO_PORT:-30092}"

API="${API:-http://${NODE_IP}:${API_PORT}/api/v1}"
MINIO_BASE="${MINIO_BASE:-http://${NODE_IP}:${MINIO_PORT}/vmtools}"
IMAGE="${IMAGE:-}"

echo "=== ci-k3s-e2e (API=${API}) ==="

echo "Waiting for API health..."
for i in $(seq 1 60); do
  if curl -sf "${API}/health" >/dev/null 2>&1; then
    echo "  API healthy after ${i} attempts"
    break
  fi
  if [[ "${i}" -eq 60 ]]; then
    echo "ERROR: API health check timed out" >&2
    kubectl -n "${NAMESPACE}" get pods
    exit 1
  fi
  sleep 5
done

echo "GET /health"
curl -sf "${API}/health" | python3 -m json.tool | head -20

echo "GET /config"
curl -sf "${API}/config" | python3 -m json.tool | head -40

UI_PORT="${UI_PORT:-30081}"
echo "GET UI (http://${NODE_IP}:${UI_PORT}/)"
curl -sf -o /dev/null -w "UI HTTP %{http_code}\n" "http://${NODE_IP}:${UI_PORT}/"

echo "Helm pod readiness..."
for dep in postgresql redis zyvor-api guestkit-worker zyvor-ui minio; do
  kubectl -n "${NAMESPACE}" rollout status "deployment/${dep}" --timeout=300s
done

if [[ -z "${IMAGE}" ]]; then
  E2E_DIR="/var/lib/zyvor/images/e2e"
  sudo mkdir -p "${E2E_DIR}"
  IMAGE="${E2E_DIR}/cirros.img"
  if [[ ! -f "${IMAGE}" ]]; then
    echo "Downloading cirros test image..."
    sudo curl -fsSL -o "${IMAGE}" \
      "https://download.cirros-cloud.net/0.6.2/cirros-0.6.2-x86_64-disk.img"
    sudo chmod 644 "${IMAGE}"
  fi
fi

echo "Running e2e-smoke.sh (IMAGE=${IMAGE})..."
API="${API}" IMAGE="${IMAGE}" bash "${ROOT}/deploy/scripts/e2e-smoke.sh"

echo "VM tools API checks..."
curl -sf "${API}/vmtools/bundle" | python3 -m json.tool | head -30
curl -sf "${API}/vmtools/coverage" | python3 -m json.tool | head -30
curl -sf "${API}/vmtools/policy" | python3 -m json.tool | head -30

echo "POST /vmtools/policy/reconcile"
curl -sf -X POST "${API}/vmtools/policy/reconcile" | python3 -m json.tool | head -30

# Both artifact names must be served: canonical for new tooling, legacy
# for pre-rebrand updaters bootstrapping from this bundle.
for name in guestkitd zyvor-guest-agent; do
  AGENT_URL="${MINIO_BASE}/linux/${name}"
  echo "Agent binary sanity (${AGENT_URL})..."
  HTTP_CODE="$(curl -sf -o "/tmp/${name}" -w '%{http_code}' "${AGENT_URL}" || echo 000)"
  if [[ "${HTTP_CODE}" != "200" ]]; then
    echo "ERROR: expected 200 from agent binary URL ${AGENT_URL}, got ${HTTP_CODE}" >&2
    exit 1
  fi
  if ! file "/tmp/${name}" | grep -qE 'ELF|executable'; then
    echo "ERROR: ${name} does not look like an ELF executable" >&2
    file "/tmp/${name}"
    exit 1
  fi
  echo "  ${name} OK ($(wc -c < "/tmp/${name}") bytes)"
done

echo "Live agent e2e (protocol 1.3) on this host..."
bash "${ROOT}/deploy/scripts/e2e-agent-live.sh" /tmp/guestkitd

echo "=== ci-k3s-e2e passed ==="
