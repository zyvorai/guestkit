#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Generate branded PDFs from user .txt docs and add docs/pdf/ to the bundle.
#
# Usage: generate-user-pdfs.sh <stage-dir> <build-dir> <product-name>
#
# Logo search order (first match wins):
#   ui/public, web/public, web-ui/public, frontend/public,
#   scripts/zyvor-branding, web/ (Rust/API embed dirs)
set -euo pipefail

STAGE="${1:?stage directory}"
BUILD_DIR="${2:?build directory}"
PRODUCT="${3:?product name}"
VERSION="${4:-${V9S_PACKAGE_VERSION:-latest}}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GEN_PY="${SCRIPT_DIR}/generate-user-pdfs.py"

find_zyvor_logo() {
  local bd="$1"
  local p
  for p in \
    "${bd}/ui/public/zyvor-logo.png" \
    "${bd}/web/public/zyvor-logo.png" \
    "${bd}/web-ui/public/zyvor-logo.png" \
    "${bd}/frontend/public/zyvor-logo.png" \
    "${bd}/scripts/zyvor-branding/zyvor-logo.png" \
    "${bd}/web/zyvor-logo.png" \
    "${bd}/src/api/web/zyvor-logo.png"; do
    if [[ -f "${p}" ]]; then
      echo "${p}"
      return 0
    fi
  done
  return 1
}

LOGO="$(find_zyvor_logo "${BUILD_DIR}")" || {
  echo "ERROR: zyvor-logo.png not found under ${BUILD_DIR} (ui/public or scripts/zyvor-branding)" >&2
  exit 1
}

VENV="${BUILD_DIR}/.pdf-venv"
PY=""
if [[ -x "${VENV}/bin/python" ]] && "${VENV}/bin/python" -c 'import fpdf' 2>/dev/null; then
  PY="${VENV}/bin/python"
elif python3 -c 'import fpdf' 2>/dev/null; then
  PY="python3"
else
  echo "  › Preparing PDF toolchain (fpdf2)…"
  rm -rf "${VENV}"
  if python3 -m venv "${VENV}" 2>/dev/null && [[ -x "${VENV}/bin/python" ]]; then
    "${VENV}/bin/python" -m pip install -q --disable-pip-version-check fpdf2
    PY="${VENV}/bin/python"
  elif python3 -m pip install --user -q fpdf2 2>/dev/null; then
    PY="python3"
  else
    echo "ERROR: cannot install fpdf2 (need python3-venv or pip)" >&2
    exit 1
  fi
fi

echo "  › User PDFs (${PRODUCT} ${VERSION}) — logo: ${LOGO#${BUILD_DIR}/}"
"${PY}" "${GEN_PY}" "${STAGE}" "${PRODUCT}" "${LOGO}" --version "${VERSION}"
