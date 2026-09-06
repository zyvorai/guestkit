#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Verify Kubernetes cluster prerequisites for VMRogue / v9s client bundles.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"
export PKG_INSTALL_ROOT="${ROOT}"
# shellcheck source=/dev/null
[[ -f "${ROOT}/.package-lib/package-ui.sh" ]] && source "${ROOT}/.package-lib/package-ui.sh"

[[ "${1:-}" == "-h" || "${1:-}" == "--help" ]] && {
  pkg_script_help "test-cluster.sh"
  exit 0
}
# shellcheck source=/dev/null
[[ -f "${ROOT}/cluster/env.sh" ]] && source "${ROOT}/cluster/env.sh"

PRODUCT="${PRODUCT:-Kubernetes client}"
APP_PORT="${APP_PORT:-5151}"
ENV_PREFIX="${ENV_PREFIX:-VMROGUE}"
KUBECONFIG="${KUBECONFIG:-${HOME}/.kube/config}"

_PKG_SESSION_START=${SECONDS}
pkg_counters_reset
pkg_banner "${PRODUCT} cluster test" "KUBECONFIG=${KUBECONFIG}"

if ! command -v kubectl >/dev/null 2>&1; then
  pkg_fail "kubectl not in PATH — run ./install-client-deps.sh"
  pkg_summary "Cluster test"
  exit 1
fi
export KUBECONFIG
if ! kubectl cluster-info >/dev/null 2>&1; then
  pkg_fail "Cannot reach API server — check KUBECONFIG and network"
  pkg_summary "Cluster test"
  exit 1
fi
pkg_ok "kubectl → API server"

if kubectl get crd virtualmachines.kubevirt.io >/dev/null 2>&1; then
  pkg_ok "KubeVirt CRD"
  phase=$(kubectl get kubevirt kubevirt -n kubevirt -o jsonpath='{.status.phase}' 2>/dev/null || true)
  if [[ "${phase}" == "Deployed" ]]; then
    pkg_ok "KubeVirt phase=Deployed"
  else
    pkg_warn "KubeVirt phase=${phase:-unknown} — run ./install-cluster.sh or wait"
  fi
else
  pkg_fail "KubeVirt missing — ./install-cluster.sh"
fi

if kubectl get crd datavolumes.cdi.kubevirt.io >/dev/null 2>&1; then
  pkg_ok "CDI DataVolume CRD"
  if kubectl get cdi cdi -o jsonpath='{.status.conditions[?(@.type=="Available")].status}' 2>/dev/null | grep -qi true; then
    pkg_ok "CDI Available"
  else
    pkg_warn "CDI not yet Available"
  fi
else
  pkg_warn "CDI not installed (optional for basic VMs)"
  pkg_detail "Install: ./install-cluster.sh or ${ENV_PREFIX}_SKIP_CDI=1"
fi

if kubectl get crd ciliumnetworkpolicies.cilium.io >/dev/null 2>&1; then
  pkg_ok "Cilium CRDs"
  if kubectl get crd ciliumclusterwidenetworkpolicies.cilium.io >/dev/null 2>&1; then
    if kubectl get ciliumclusterwidenetworkpolicy allow-kubevirt-virt-launcher-egress >/dev/null 2>&1; then
      pkg_ok "virt-launcher egress CCNP"
    else
      pkg_warn "CCNP missing — VM internet may fail: ./apply-cluster-network.sh"
    fi
  fi
else
  pkg_skip "Cilium not detected (non-Cilium CNI)"
fi

if kubectl get deployment metrics-server -n kube-system >/dev/null 2>&1; then
  pkg_ok "metrics-server"
else
  pkg_skip "metrics-server (optional)"
fi

if [[ -n "${APP_NAMESPACE:-}" ]] && kubectl get namespace "${APP_NAMESPACE}" >/dev/null 2>&1; then
  pkg_ok "namespace ${APP_NAMESPACE}"
else
  pkg_skip "namespace ${APP_NAMESPACE:-<unset>} — create when deploying in-cluster"
fi

pkg_summary "Cluster readiness"
[[ "${_PKG_COUNTERS_FAIL}" -eq 0 ]]
