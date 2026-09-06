#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Zyvor VM Services — kind + KubeVirt quickstart
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CLUSTER_NAME="${CLUSTER_NAME:-zyvor}"
NAMESPACE="${NAMESPACE:-zyvor}"

echo "=== Zyvor VM Services Quickstart ==="

if ! command -v kind >/dev/null; then
  echo "kind is required: https://kind.sigs.k8s.io/"
  exit 1
fi

if ! command -v kubectl >/dev/null; then
  echo "kubectl is required"
  exit 1
fi

if ! command -v helm >/dev/null; then
  echo "helm is required"
  exit 1
fi

# Create kind cluster if missing
if ! kind get clusters | grep -q "^${CLUSTER_NAME}$"; then
  echo "Creating kind cluster ${CLUSTER_NAME}..."
  cat <<EOF | kind create cluster --name "${CLUSTER_NAME}" --config=-
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
nodes:
  - role: control-plane
    extraMounts:
      - hostPath: /dev/kvm
        containerPath: /dev/kvm
EOF
fi

kubectl cluster-info --context "kind-${CLUSTER_NAME}"

# Install KubeVirt (pinned example version)
KUBEVIRT_VERSION="${KUBEVIRT_VERSION:-v1.4.0}"
if ! kubectl get crd virtualmachines.kubevirt.io >/dev/null 2>&1; then
  echo "Installing KubeVirt ${KUBEVIRT_VERSION}..."
  kubectl apply -f "https://github.com/kubevirt/kubevirt/releases/download/${KUBEVIRT_VERSION}/kubevirt-operator.yaml"
  kubectl apply -f "https://github.com/kubevirt/kubevirt/releases/download/${KUBEVIRT_VERSION}/kubevirt-cr.yaml"
  kubectl -n kubevirt wait kv kubevirt --for condition=Available --timeout=300s || true
fi

# Build images
echo "Building container images..."
docker build -f "${ROOT}/crates/guestkit-worker/Dockerfile" -t guestkit-worker:latest "${ROOT}"
docker build -f "${ROOT}/crates/zyvor-api/Dockerfile" -t zyvor-api:latest "${ROOT}"
docker build -f "${ROOT}/deploy/ui/Dockerfile" -t zyvor-ui:latest "${ROOT}/deploy/ui"

kind load docker-image guestkit-worker:latest --name "${CLUSTER_NAME}"
kind load docker-image zyvor-api:latest --name "${CLUSTER_NAME}"
kind load docker-image zyvor-ui:latest --name "${CLUSTER_NAME}"

# Install nginx ingress for kind
if ! kubectl get ns ingress-nginx >/dev/null 2>&1; then
  kubectl apply -f https://raw.githubusercontent.com/kubernetes/ingress-nginx/main/deploy/static/provider/kind/deploy.yaml
  kubectl wait --namespace ingress-nginx --for=condition=ready pod --selector=app.kubernetes.io/component=controller --timeout=180s
fi

# Deploy Zyvor Helm chart
helm upgrade --install zyvor "${ROOT}/deploy/helm/zyvor" \
  -n "${NAMESPACE}" --create-namespace \
  --set persistence.vmImages.storageClass=standard \
  --set guestkitWorker.image=guestkit-worker:latest \
  --set zyvorApi.image=zyvor-api:latest \
  --set zyvorUi.image=zyvor-ui:latest

kubectl -n "${NAMESPACE}" rollout status deployment/zyvor-api --timeout=180s
kubectl -n "${NAMESPACE}" rollout status deployment/guestkit-worker --timeout=180s

echo ""
echo "=== Deployment ready ==="
echo "Add to /etc/hosts:"
echo "  127.0.0.1 console.zyvor.local api.zyvor.local"
echo ""
echo "Port-forward UI (if ingress not configured):"
echo "  kubectl -n ${NAMESPACE} port-forward svc/zyvor-ui 8081:80"
echo ""
echo "API health:"
echo "  curl http://api.zyvor.local/api/v1/health"
echo ""
echo "Smoke test (requires sample image at ./vms/test.qcow2):"
echo "  API=http://api.zyvor.local/api/v1"
echo "  curl -F 'file=@./vms/test.qcow2' \$API/vms/import"
