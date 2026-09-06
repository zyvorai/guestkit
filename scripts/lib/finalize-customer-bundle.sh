#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Finalize customer tarball: branded PDFs, welcome page, path verification.
# Usage: finalize-customer-bundle.sh <stage> <build-dir> <product> [version]
set -euo pipefail

STAGE="${1:?stage directory}"
BUILD_DIR="${2:?build directory}"
PRODUCT="${3:?product name}"
VERSION="${4:-${V9S_PACKAGE_VERSION:-latest}}"
LIB="${BUILD_DIR}/scripts/lib"

for tool in generate-customer-pdfs.sh verify-bundle-script-paths.sh; do
  [[ -x "${LIB}/${tool}" ]] || { echo "ERROR: missing ${LIB}/${tool}" >&2; exit 1; }
done

chmod +x "${LIB}/generate-customer-pdfs.sh" "${LIB}/verify-bundle-script-paths.sh"
"${LIB}/generate-customer-pdfs.sh" "${STAGE}" "${BUILD_DIR}" "${PRODUCT}" "${VERSION}"
"${LIB}/verify-bundle-script-paths.sh" "${STAGE}"

test -f "${STAGE}/docs/welcome.html" || { echo "ERROR: missing docs/welcome.html" >&2; exit 1; }
test -f "${STAGE}/docs/pdf/WELCOME.pdf" || { echo "ERROR: missing docs/pdf/WELCOME.pdf" >&2; exit 1; }
test -f "${STAGE}/OPEN_FIRST.txt" || { echo "ERROR: missing OPEN_FIRST.txt" >&2; exit 1; }
