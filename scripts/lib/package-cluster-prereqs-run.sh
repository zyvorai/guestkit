#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Install cluster prerequisites: Cilium (when applicable), metrics-server, KubeVirt, CDI.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"
export PKG_INSTALL_ROOT="${ROOT}"
# shellcheck source=/dev/null
[[ -f "${ROOT}/.package-lib/package-ui.sh" ]] && source "${ROOT}/.package-lib/package-ui.sh"

[[ "${1:-}" == "-h" || "${1:-}" == "--help" ]] && {
  pkg_script_help "install-cluster.sh"
  exit 0
}
# shellcheck source=/dev/null
[[ -f "${ROOT}/cluster/env.sh" ]] && source "${ROOT}/cluster/env.sh"

SCRIPT="${ROOT}/cluster/install-cluster-prereqs.sh"
[[ -f "${SCRIPT}" ]] || { echo "ERROR: missing ${SCRIPT}" >&2; exit 1; }

_map() {
  local name="$1" v9s="V9S_${name}" vmr="VMROGUE_${name}"
  [[ -z "${!v9s:-}" && -n "${!vmr:-}" ]] && export "${v9s}=${!vmr}"
}
for _v in SKIP_CILIUM SKIP_CDI SKIP_KUBEVIRT SKIP_METRICS_SERVER SKIP_MULTUS \
  CILIUM_CHART_VERSION KUBEVIRT_VERSION CDI_VERSION INSTALL_METRICS_SERVER \
  INSTALL_MULTUS INSTALL_SNAPSHOT_CONTROLLER INSTALL_METALLB INSTALL_PROMETHEUS; do
  _map "${_v}"
done

pkg_banner "${PRODUCT:-Cluster} prerequisites" "Cilium · metrics-server · KubeVirt · CDI"
pkg_info "Flags: V9S_* and VMROGUE_* (see CLUSTER_SETUP.txt)"
pkg_detail "Skip examples: V9S_SKIP_CILIUM=1 V9S_SKIP_CDI=1"
echo ""
pkg_phase "Installer"
pkg_info "Running cluster/install-cluster-prereqs.sh (may take 10–20 minutes)…"
echo ""
exec bash "${SCRIPT}" "$@"
