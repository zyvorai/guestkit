#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Deploy Zyvor VM Services to a k3s host (build with podman, helm install).
# Run ON the remote host or GHA runner after guestkit sources are synced.
#
# Usage:
#   bash deploy/scripts/deploy-remote-k3s.sh
#   ROOT=/path/to/guestkit bash deploy/scripts/deploy-remote-k3s.sh
#   PULL_REGISTRY=ghcr.io/zyvorai IMAGE_TAG=v1.2.3 bash deploy/scripts/deploy-remote-k3s.sh
#   HELM_VALUES_FILE=values-ci.yaml bash deploy/scripts/deploy-remote-k3s.sh
#   SKIP_K3S_INSTALL=1 bash deploy/scripts/deploy-remote-k3s.sh
set -euo pipefail

ROOT="${ROOT:-$(cd "$(dirname "$0")/../.." && pwd)}"
NAMESPACE="${NAMESPACE:-zyvor}"
K3S_BIN="${K3S_BIN:-/usr/local/bin/k3s}"
BUILDER="${BUILDER:-podman}"
PULL_REGISTRY="${PULL_REGISTRY:-}"
IMAGE_TAG="${IMAGE_TAG:-latest}"
NO_CACHE="${NO_CACHE:-}"
HELM_VALUES_FILE="${HELM_VALUES_FILE:-values-k3s.yaml}"
SKIP_K3S_INSTALL="${SKIP_K3S_INSTALL:-}"

cd "${ROOT}"

if [[ -z "${SKIP_K3S_INSTALL}" ]]; then
  bash "${ROOT}/deploy/scripts/install-k3s-ubuntu.sh"
fi

if [[ -f "${HOME}/.kube/config" ]]; then
  export KUBECONFIG="${KUBECONFIG:-${HOME}/.kube/config}"
fi

if ! command -v "${BUILDER}" >/dev/null; then
  if command -v docker >/dev/null; then
    BUILDER=docker
  else
    echo "ERROR: ${BUILDER} not found (install podman or set BUILDER=docker)"
    exit 1
  fi
fi
if ! command -v helm >/dev/null; then
  echo "ERROR: helm not found"
  exit 1
fi
if ! command -v kubectl >/dev/null; then
  echo "ERROR: kubectl not found"
  exit 1
fi

VALUES_PATH="${ROOT}/deploy/helm/zyvor/${HELM_VALUES_FILE}"
if [[ ! -f "${VALUES_PATH}" ]]; then
  echo "ERROR: Helm values file not found: ${VALUES_PATH}"
  exit 1
fi

echo "=== Zyvor k3s deploy (root=${ROOT}, values=${HELM_VALUES_FILE}) ==="

resolve_node_ip() {
  local ip
  ip="$(kubectl get nodes -o jsonpath='{.items[0].status.addresses[?(@.type=="InternalIP")].address}' 2>/dev/null || true)"
  if [[ -z "${ip}" ]]; then
    ip="$(hostname -I 2>/dev/null | awk '{print $1}')"
  fi
  echo "${ip}"
}

NODE_IP="${NODE_IP:-$(resolve_node_ip)}"
API_NODE_PORT="${API_NODE_PORT:-30080}"
MINIO_NODE_PORT="${MINIO_NODE_PORT:-30092}"

ZEUS_PUBLIC_URL="${ZEUS_PUBLIC_URL:-http://${NODE_IP}:${API_NODE_PORT}}"
VMTOOLS_BASE_URL="${VMTOOLS_BASE_URL:-http://${NODE_IP}:${MINIO_NODE_PORT}/vmtools}"
GUESTKIT_BINARY_URL="${GUESTKIT_BINARY_URL:-http://${NODE_IP}:${MINIO_NODE_PORT}/vmtools/linux/zyvor-guest-agent}"

build_and_import() {
  local name="$1"
  local dockerfile="$2"
  local context="$3"
  local tar="${HOME}/zyvor-${name//:/-}.tar"
  echo "Building ${name}..."
  local cache_flag=()
  if [[ -n "${NO_CACHE}" && "${name}" == zyvor-api ]]; then
    cache_flag=(--no-cache)
    echo "  (no-cache rebuild for zyvor-api)"
  fi
  "${BUILDER}" build --format docker "${cache_flag[@]}" -t "${name}:latest" -f "${dockerfile}" "${context}"
  echo "Exporting ${name}..."
  rm -f "${tar}"
  (cd /tmp && "${BUILDER}" save "${name}:latest" -o "${tar}")
  echo "Importing ${name} into k3s..."
  sudo "${K3S_BIN}" ctr -n k8s.io images import "${tar}"
  local base="${name%%:*}"
  local tag="${name#*:}"
  if [[ "${base}" == "${tag}" ]]; then
    tag="latest"
  fi
  local local_ref="localhost/${base}:${tag}"
  local k8s_ref="docker.io/library/${base}:${tag}"
  sudo "${K3S_BIN}" ctr -n k8s.io images tag "${local_ref}" "${k8s_ref}" 2>/dev/null \
    || sudo "${K3S_BIN}" ctr -n k8s.io images tag "${base}:${tag}" "${k8s_ref}" 2>/dev/null \
    || true
  rm -f "${tar}"
}

if [[ -n "${PULL_REGISTRY}" ]]; then
  WORKER_IMAGE="${PULL_REGISTRY}/guestkit-worker:${IMAGE_TAG}"
  API_IMAGE="${PULL_REGISTRY}/zyvor-api:${IMAGE_TAG}"
  UI_IMAGE="${PULL_REGISTRY}/zyvor-ui:${IMAGE_TAG}"
  echo "Pulling published images from ${PULL_REGISTRY}..."
  for img in "${WORKER_IMAGE}" "${API_IMAGE}" "${UI_IMAGE}"; do
    sudo "${K3S_BIN}" ctr -n k8s.io images pull "${img}"
  done
else
  WORKER_IMAGE="guestkit-worker:latest"
  API_IMAGE="zyvor-api:latest"
  UI_IMAGE="zyvor-ui:latest"
  build_and_import guestkit-worker "${ROOT}/crates/guestkit-worker/Dockerfile" "${ROOT}"
  build_and_import zyvor-api "${ROOT}/crates/zyvor-api/Dockerfile" "${ROOT}"
  build_and_import zyvor-ui "${ROOT}/deploy/ui/Dockerfile" "${ROOT}/deploy/ui"
fi

if [[ -f "${ROOT}/deploy/crd/zeus-vmtools.yaml" ]]; then
  echo "Applying Zeus VM Tools CRDs..."
  kubectl apply -f "${ROOT}/deploy/crd/zeus-vmtools.yaml"
fi

echo "Installing Helm chart..."
helm upgrade --install zyvor "${ROOT}/deploy/helm/zyvor" \
  -n "${NAMESPACE}" --create-namespace \
  -f "${VALUES_PATH}" \
  --set guestkitWorker.image="${WORKER_IMAGE}" \
  --set zyvorApi.image="${API_IMAGE}" \
  --set zyvorUi.image="${UI_IMAGE}" \
  --set zyvorApi.zeusPublicUrl="${ZEUS_PUBLIC_URL}" \
  --set zyvorApi.guestkitBinaryUrl="${GUESTKIT_BINARY_URL}" \
  --set vmtools.bundle.baseUrl="${VMTOOLS_BASE_URL}" \
  --set minio.service.nodePort="${MINIO_NODE_PORT}"

if [[ -n "${NO_CACHE}" ]] || [[ -z "${PULL_REGISTRY}" ]]; then
  echo "Restarting API, UI, and worker pods to pick up freshly built :latest images..."
  kubectl -n "${NAMESPACE}" rollout restart deployment/zyvor-api deployment/zyvor-ui deployment/guestkit-worker
fi

echo "Waiting for rollouts..."
kubectl -n "${NAMESPACE}" rollout status deployment/postgresql --timeout=180s
kubectl -n "${NAMESPACE}" rollout status deployment/redis --timeout=180s
kubectl -n "${NAMESPACE}" rollout status deployment/zyvor-api --timeout=300s
kubectl -n "${NAMESPACE}" rollout status deployment/guestkit-worker --timeout=300s
kubectl -n "${NAMESPACE}" rollout status deployment/zyvor-ui --timeout=180s

API_PORT="$(kubectl -n "${NAMESPACE}" get svc zyvor-api -o jsonpath='{.spec.ports[0].nodePort}')"
UI_PORT="$(kubectl -n "${NAMESPACE}" get svc zyvor-ui -o jsonpath='{.spec.ports[0].nodePort}')"

echo ""
echo "=== Zyvor deployed ==="
kubectl -n "${NAMESPACE}" get pods
echo ""
echo "API:  http://${NODE_IP}:${API_PORT}/api/v1/health"
echo "UI:   http://${NODE_IP}:${UI_PORT}/"
echo ""
echo "Smoke (empty image will fail doctor — use a real disk):"
echo "  API=http://${NODE_IP}:${API_PORT}/api/v1"
echo "  curl -sf \"\$API/health\""
