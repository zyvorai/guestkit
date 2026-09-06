#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Install k3s single-node and deploy prerequisites on Ubuntu (CI or remote host).
# Idempotent: safe to re-run.
set -euo pipefail

K3S_BIN="${K3S_BIN:-/usr/local/bin/k3s}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${HOME}/.kube/config}"

echo "=== install-k3s-ubuntu.sh ==="

if ! command -v k3s >/dev/null 2>&1; then
  echo "Installing k3s..."
  curl -sfL https://get.k3s.io | INSTALL_K3S_EXEC="--write-kubeconfig-mode 644" sh -
else
  echo "k3s already installed"
fi

mkdir -p "$(dirname "${KUBECONFIG_PATH}")"
if [[ -f /etc/rancher/k3s/k3s.yaml ]]; then
  sudo cp /etc/rancher/k3s/k3s.yaml "${KUBECONFIG_PATH}"
  sudo chown "$(id -u)":"$(id -g)" "${KUBECONFIG_PATH}"
  chmod 600 "${KUBECONFIG_PATH}"
fi
export KUBECONFIG="${KUBECONFIG_PATH}"

if ! command -v kubectl >/dev/null 2>&1; then
  if [[ -x "${K3S_BIN}" ]]; then
    echo "kubectl via k3s"
    sudo ln -sf "${K3S_BIN}" /usr/local/bin/kubectl 2>/dev/null || true
  fi
fi

if ! command -v helm >/dev/null 2>&1; then
  echo "Installing helm..."
  curl -fsSL https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash
fi

echo "Installing qemu-utils..."
sudo apt-get update -qq
sudo apt-get install -y qemu-utils curl musl-tools gcc-mingw-w64-x86-64

if ! command -v podman >/dev/null 2>&1 && ! command -v docker >/dev/null 2>&1; then
  echo "Installing podman..."
  sudo apt-get install -y podman
fi

# guestkit-worker's pod bind-mounts the host's /dev (hostPath, HostToContainer
# propagation), so device node permissions here are what the worker actually
# sees. /dev/loopN and /dev/nbdN are created root:disk 0660 by udev — the
# worker's plain (non-root) mount attempt then fails with EACCES, which
# guestkit's device wait-loop can't distinguish from "not ready yet". Same
# fix as ci.yml's "Setup loop and NBD devices" step, applied here since this
# script never got it.
echo "Setting up loop and NBD devices..."
sudo modprobe loop max_part=8 || true
sudo chmod 666 /dev/loop-control 2>/dev/null || true
sudo chmod 666 /dev/loop[0-9]* 2>/dev/null || true
sudo modprobe nbd max_part=8 || true
sudo chmod 666 /dev/nbd[0-9]* 2>/dev/null || true

sudo mkdir -p /var/lib/zyvor/images
sudo chmod 1777 /var/lib/zyvor/images

echo "Waiting for k3s node..."
for _ in $(seq 1 60); do
  if kubectl get nodes >/dev/null 2>&1; then
    break
  fi
  sleep 2
done
kubectl get nodes -o wide

# values-ci.yaml sets kubevirt.enabled: true (zyvor-api/worker assume the
# KubeVirt CRDs exist — RBAC, mTLS, the kube-list-virtualmachines calls
# behind /vmtools/coverage and the fleet endpoints), but nothing installed
# the KubeVirt operator that actually registers those CRDs with the API
# server — same install this repo already uses in
# deploy/scripts/kind-kubevirt-quickstart.sh, just never ported to this
# script. GitHub-hosted runners have no /dev/kvm (no nested
# virtualization), so virt-handler won't reach a fully healthy state and
# actually starting a VirtualMachineInstance won't work here — but the
# CRDs register, and the kubevirt.io/v1 API group routes real (empty)
# responses instead of 404 as soon as the operator applies them, which is
# all this E2E job's default (non-E2E_KUBEVIRT) path needs.
KUBEVIRT_VERSION="${KUBEVIRT_VERSION:-v1.4.0}"
if ! kubectl get crd virtualmachines.kubevirt.io >/dev/null 2>&1; then
  echo "Installing KubeVirt ${KUBEVIRT_VERSION}..."
  kubectl apply -f "https://github.com/kubevirt/kubevirt/releases/download/${KUBEVIRT_VERSION}/kubevirt-operator.yaml"
  kubectl apply -f "https://github.com/kubevirt/kubevirt/releases/download/${KUBEVIRT_VERSION}/kubevirt-cr.yaml"
  echo "Waiting for KubeVirt CRDs to register..."
  for _ in $(seq 1 60); do
    if kubectl get crd virtualmachines.kubevirt.io >/dev/null 2>&1; then
      break
    fi
    sleep 2
  done
  # Best-effort only: no /dev/kvm here, so virt-handler can't fully come up.
  kubectl -n kubevirt wait kv kubevirt --for condition=Available --timeout=120s || true
else
  echo "KubeVirt CRDs already present"
fi

echo "=== k3s ready (KUBECONFIG=${KUBECONFIG}) ==="
