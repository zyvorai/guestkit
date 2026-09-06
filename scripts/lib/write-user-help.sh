#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Generate HELP.txt for a user tarball directory.
# Usage: write-user-help.sh <stage-dir> <product-name> <kind>
#   kind: k8s | host | platform | minimal
set -euo pipefail

STAGE="${1:?stage directory}"
PRODUCT="${2:?product name}"
KIND="${3:-minimal}"

case "${KIND}" in
  k8s | host | platform | minimal) ;;
  *)
    echo "write-user-help.sh: unknown kind '${KIND}' (use k8s|host|platform|minimal)" >&2
    exit 1
    ;;
esac

mkdir -p "${STAGE}"

{
  cat <<EOF
================================================================================
  ${PRODUCT} — user bundle help
  zyvor.dev · HyperSDK · © 2026
================================================================================

START HERE
  OPEN_FIRST.txt      ✦ open this first (points to welcome page)
  docs/welcome.html   interactive offline guide (best experience)
  docs/pdf/WELCOME.pdf
  cat START_HERE.txt
  ./install-everything.sh     recommended full automatic install
  ./install-everything.sh --help
  ./install.sh --help

DOCUMENTATION (read in this order)
  HELP.txt            all scripts explained (this file)
  docs/pdf/           same guides as printable PDFs (Zyvor logo on each page)
  docs/PDF_INDEX.txt  list of PDF filenames
  ZYVOR_INSTALL.txt   fastest install, no compile on this machine
  QUICKSTART.txt      step-by-step commands for ${PRODUCT}
  README.txt          archive contents and requirements
EOF

  echo ""
  echo "LICENSE & LEGAL (read before install)"
  if [[ -f "${STAGE}/LICENSE" ]]; then
    echo "  LICENSE                 cat LICENSE — software license"
  fi
  if [[ -f "${STAGE}/LICENSE.txt" ]]; then
    echo "  LICENSE.txt             cat LICENSE.txt — software license"
  fi
  if [[ -f "${STAGE}/ZYVOR-COMPANY-TERMS.md" ]]; then
    echo "  ZYVOR-COMPANY-TERMS.md  Zyvor distribution terms (install prompts ACCEPT)"
  fi
  if [[ -f "${STAGE}/LEGAL-INDEX.txt" ]]; then
    echo "  LEGAL-INDEX.txt         index of all legal files"
  fi
  if [[ -d "${STAGE}/docs/legal" ]]; then
    echo "  docs/legal/             company reference"
  fi
  echo "  ./install.sh --help     license summary + install options"

  case "${KIND}" in
    k8s)
      cat <<'EOF'
  CORE_KUBERNETES.txt Kubernetes dashboard URL, ports (:30050 vs :5050 vs :5092)
  PREREQUISITES.txt   checklist before install
  CLUSTER_SETUP.txt   cluster bootstrap flags and order
EOF
      ;;
    host)
      cat <<'EOF'
  PREREQUISITES.txt   checklist before install
  HOST_SETUP.txt      libvirt/KVM host setup and troubleshooting
EOF
      ;;
    platform)
      cat <<'EOF'
  PREREQUISITES.txt   checklist (if bundled)
EOF
      ;;
  esac

  cat <<'EOF'

--------------------------------------------------------------------------------
QUICK START (client on this machine)
--------------------------------------------------------------------------------
  tar xzf PRODUCT-*-linux-amd64.tar.gz
  cd PRODUCT-*-linux-amd64
  cat START_HERE.txt
  ./install-everything.sh

  The installer prints a URL using this server's LAN IP (not only localhost).

--------------------------------------------------------------------------------
SCRIPTS — install & remove
--------------------------------------------------------------------------------

=== install-everything.sh ===
  Full automatic install: OS deps, config, binary checks, optional host/cluster
  tests, and production setup when bundled (e.g. Machina systemd).

  ./install-everything.sh
  ./install-everything.sh --help
  ./install-everything.sh --kubeconfig /path/to/config    (Kubernetes products)

  Environment:
    ZYVOR_KUBECONFIG=/path/to/config     same as --kubeconfig
    ZYVOR_NONINTERACTIVE=1               no kubeconfig prompt (CI / scripts)
    ZYVOR_AUTO_INSTALL=0                 skip bundled install-full.sh (Machina)

=== install.sh ===
  Core client install (same steps as install-everything, without extra phases).

  ./install.sh
  ./install.sh --help
  ./install.sh --kubeconfig /path/to/config

=== install-client-deps.sh ===
  Installs OS packages (kubectl, libvirt, Python venv deps, etc.) for this product.
  Usually run automatically by install.sh; run alone if deps failed:

  sudo ./install-client-deps.sh

=== uninstall.sh ===
  Stop processes, remove config created by install, optionally delete this folder.

  ./uninstall.sh --help
  ./uninstall.sh --yes
  ./uninstall.sh --yes --remove-dir
  ./uninstall.sh --yes --keep-config

=== test-package.sh ===
  Smoke test: binaries, optional API health, optional cluster reachability.

  ./test-package.sh
  ./test-package.sh --help

EOF

  case "${KIND}" in
    k8s)
      cat <<'EOF'
--------------------------------------------------------------------------------
SCRIPTS — Kubernetes cluster (once per cluster; needs admin kubeconfig)
--------------------------------------------------------------------------------

=== install-cluster.sh ===
  Bootstrap Cilium, KubeVirt, CDI (product-specific flags — see CLUSTER_SETUP.txt).

  export KUBECONFIG=/path/to/admin/kubeconfig
  ./install-cluster.sh
  ./install-cluster.sh --help

  Skip flags (examples): PRODUCT_SKIP_CDI=1  PRODUCT_SKIP_KUBEVIRT=1
  See CLUSTER_SETUP.txt for full list.

=== apply-cluster-network.sh ===
  Cilium egress / network bootstrap for virt-launcher and platform pods.

  ./apply-cluster-network.sh
  VMROGUE_SKIP_CILIUM_EGRESS_BOOTSTRAP=1 ./apply-cluster-network.sh   (skip)

=== test-cluster.sh ===
  Verify kubectl, KubeVirt, CDI, and API reachability.

  ./test-cluster.sh
  ./test-cluster.sh --help

Kubeconfig (client install)
  Install auto-detects, in order:
    $KUBECONFIG, ~/.kube/config, /etc/rancher/k3s/k3s.yaml,
    /etc/kubernetes/admin.conf, MicroK8s client.config
  Override: ./install.sh --kubeconfig /path/to/config

EOF
      ;;
    host)
      cat <<'EOF'
--------------------------------------------------------------------------------
SCRIPTS — hypervisor host (libvirt / KVM — not Kubernetes)
--------------------------------------------------------------------------------

=== test-host.sh ===
  Preflight: KVM, libvirtd, tools, permissions.

  ./test-host.sh
  ./test-host.sh --help

=== install-full.sh ===
  Production host setup: systemd service, TLS, firewall (Machina and similar).

  sudo ./install-full.sh --open-firewall
  sudo ./install-full.sh --help
  sudo ./install-full.sh --bind 0.0.0.0

  Flags: --deps-only  --open-firewall  --bind ADDR

EOF
      ;;
    platform)
      cat <<'EOF'
--------------------------------------------------------------------------------
PLATFORM NOTES (HyperSDK / hyper2kvm)
--------------------------------------------------------------------------------
  Config: ~/.config/hypersdk/config.yaml or *.env in this folder (see README.txt)
  Dashboard: often https://<host>:5080/web/dashboard/  (subpath — use printed URL)
  CLI: ./bin/hyperctl --help  ./bin/hypervisord --help  (if bundled)

EOF
      ;;
  esac

  cat <<EOF
--------------------------------------------------------------------------------
TYPICAL ORDER
--------------------------------------------------------------------------------
EOF

  case "${KIND}" in
    k8s)
      if [[ "${PRODUCT}" == "v9s" ]]; then
        cat <<EOF
  Cluster (once):  install-cluster.sh → deploy v9s in cluster → apply-cluster-network.sh
                   Dashboard: https://<node-ip>:30050/vms  (see CORE_KUBERNETES.txt)
  This machine:    install-everything.sh → edit v9s.env → test-cluster.sh → test-package.sh
EOF
      else
        cat <<EOF
  Cluster (once):  install-cluster.sh → deploy ${PRODUCT} in cluster → apply-cluster-network.sh
  This machine:    install-everything.sh → edit *.env → start product → test-package.sh
EOF
      fi
      ;;
    host)
      cat <<EOF
  This machine:    install-everything.sh  (or install.sh → test-host.sh → sudo install-full.sh)
EOF
      ;;
    platform)
      cat <<EOF
  This machine:    install-everything.sh → edit config → start daemon/API → test-package.sh
EOF
      ;;
    minimal)
      cat <<EOF
  This machine:    install-everything.sh → edit *.env → start services → test-package.sh
EOF
      ;;
  esac

  cat <<'EOF'

--------------------------------------------------------------------------------
GETTING HELP
--------------------------------------------------------------------------------
  All install scripts:     ./install.sh --help  |  ./install-everything.sh --help
  Remove install:          ./uninstall.sh --help
  Product binary:          run the main binary with --help (see README.txt)
  License:                 cat LICENSE · cat LEGAL-INDEX.txt
  Zyvor:                   https://zyvor.dev · sales@zyvor.dev · legal@zyvor.dev

--------------------------------------------------------------------------------
ENTERPRISE (fleet, SLA, VMware exit)
--------------------------------------------------------------------------------
EOF
  cat <<EOF
  Production / fleet / SLA:  https://zyvor.dev/contact?utm_source=package&utm_medium=${PRODUCT}
  Demo:                      https://zyvor.dev/demo?utm_source=package&utm_medium=${PRODUCT}
  ROI:                       https://zyvor.dev/roi?utm_source=package&utm_medium=${PRODUCT}
  Sales:                     sales@zyvor.dev · info@zyvor.dev

EOF
} > "${STAGE}/HELP.txt"

echo "wrote ${STAGE}/HELP.txt (${PRODUCT}, ${KIND})"
