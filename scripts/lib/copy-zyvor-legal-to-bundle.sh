#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Copy license + Zyvor legal pack into a release/user bundle directory.
# Usage: copy-zyvor-legal-to-bundle.sh <stage-dir> <repo-root>
set -euo pipefail

STAGE="${1:?stage directory}"
ROOT="${2:?repo root}"

mkdir -p "${STAGE}/docs/legal" "${STAGE}/legal/templates"

# Code license (LGPL, Apache, or proprietary LICENSE)
if [[ -f "${ROOT}/LICENSE" ]]; then
  cp "${ROOT}/LICENSE" "${STAGE}/LICENSE"
fi
if [[ -f "${ROOT}/NOTICE" ]]; then
  cp "${ROOT}/NOTICE" "${STAGE}/NOTICE"
fi

if [[ ! -f "${STAGE}/LICENSE" && ! -f "${STAGE}/LICENSE.txt" ]]; then
  echo "ERROR: no LICENSE or LICENSE.txt in ${ROOT}" >&2
  exit 1
fi

LEGAL_SRC="${ROOT}/docs/legal"
if [[ -d "${LEGAL_SRC}" ]]; then
  for f in "${LEGAL_SRC}"/*.md; do
    [[ -f "$f" ]] || continue
    base=$(basename "$f")
    cp "$f" "${STAGE}/docs/legal/${base}"
    cp "$f" "${STAGE}/legal/${base}"
  done
  if [[ -d "${LEGAL_SRC}/templates" ]]; then
    cp -R "${LEGAL_SRC}/templates/." "${STAGE}/legal/templates/"
    mkdir -p "${STAGE}/docs/legal/templates"
    cp -R "${LEGAL_SRC}/templates/." "${STAGE}/docs/legal/templates/"
  fi
fi

# Proprietary deploy acceptance (PacketWolf)
if [[ -f "${ROOT}/scripts/lib/license-accept.sh" ]]; then
  mkdir -p "${STAGE}/.package-lib"
  cp "${ROOT}/scripts/lib/license-accept.sh" "${STAGE}/.package-lib/"
  chmod +x "${STAGE}/.package-lib/license-accept.sh"
fi

{
  echo "ZyvorAI Labs — legal pack"
  echo "https://zyvor.dev · sales@zyvor.dev · info@zyvor.dev · legal@zyvor.dev"
  echo ""
  echo "FILES:"
  [[ -f "${STAGE}/NOTICE" ]] && echo "  NOTICE               — copyright attribution (Apache 2.0)"
  [[ -f "${STAGE}/LICENSE.txt" ]] && echo "  LICENSE.txt          — software license"
  echo "  legal/ docs/legal/   — company reference"
  echo ""
  echo "Read LICENSE / LICENSE.txt first."
} > "${STAGE}/LEGAL-INDEX.txt"

echo "Legal pack → ${STAGE}/ (LICENSE, LEGAL-INDEX.txt)"
