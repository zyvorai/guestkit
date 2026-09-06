#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Build Zeus VM Tools Windows MSI (WiX) or publish a zip scaffold.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
VERSION="${VERSION:-0.1.0}"
DIST="${ROOT}/dist/vmtools/windows"
WXS="${ROOT}/packaging/vmtools/windows/zyvor-guest-agent.wxs"
STAGING="${DIST}/staging"
TEMPLATES="${ROOT}/templates/agent"

mkdir -p "${STAGING}"

build_windows_agent() {
  echo "Building zyvor-guest-agent for Windows (standalone crate)..."
  cd "${ROOT}"
  rustup target add x86_64-pc-windows-gnu 2>/dev/null || true
  if command -v x86_64-w64-mingw32-gcc >/dev/null; then
    export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
  fi
  cargo build --release -p zyvor-guest-agent \
    --target x86_64-pc-windows-gnu
  cp "target/x86_64-pc-windows-gnu/release/guestkitd.exe" "${STAGING}/"
}

if command -v cargo >/dev/null 2>&1; then
  build_windows_agent || {
    echo "Windows cross-compile unavailable — place guestkitd.exe in ${STAGING}/ manually."
    touch "${STAGING}/.agent-missing"
  }
else
  echo "cargo not found — skipping agent build."
fi

cp "${ROOT}/packaging/vmtools/windows/install.ps1" "${STAGING}/"
cp "${ROOT}/templates/agent/windows/register-updater-task.ps1" "${STAGING}/"
cp "${TEMPLATES}/guest-agent-windows.toml" "${STAGING}/guest-agent.toml"
cp "${TEMPLATES}/agent-policy.yaml" "${STAGING}/agent-policy.yaml"

if [[ -f "${STAGING}/guestkitd.exe" ]]; then
  cp "${STAGING}/guestkitd.exe" "${DIST}/"
  cp "${STAGING}/guestkitd.exe" "${DIST}/../windows/" 2>/dev/null || true
fi

ZIP_PATH="${DIST}/zyvor-vm-tools-windows-${VERSION}.zip"
if [[ -f "${STAGING}/guestkitd.exe" ]]; then
  (cd "${STAGING}" && zip -r "${ZIP_PATH}" .)
  WINDOWS_ZIP_SHA256="$(sha256sum "${ZIP_PATH}" | awk '{print $1}')"
  echo "${WINDOWS_ZIP_SHA256}" > "${DIST}/zyvor-vm-tools-windows-${VERSION}.sha256"
  MANIFEST_JSON=$(printf '{"version":"%s","channel":"stable","windows_zip_sha256":"%s"}' "$VERSION" "$WINDOWS_ZIP_SHA256")
  WINDOWS_ZIP_SIGNATURE=$(cargo run -q -p zyvor-guest-agent -- sign-manifest "${MANIFEST_JSON}" 2>/dev/null || true)
  if [ -n "${WINDOWS_ZIP_SIGNATURE}" ]; then
    echo "${WINDOWS_ZIP_SIGNATURE}" > "${DIST}/zyvor-vm-tools-windows-${VERSION}.sig"
  fi
fi

if [[ -f "${STAGING}/guestkitd.exe" ]] && command -v candle >/dev/null && command -v light >/dev/null; then
  WIX_OBJ="${DIST}/zyvor-vm-tools.wixobj"
  cp -a "${STAGING}/"* "${DIST}/"
  sed "s/Version=\"0.1.0.0\"/Version=\"${VERSION}.0\"/" "${WXS}" > "${DIST}/zyvor-guest-agent.wxs"
  candle -out "${WIX_OBJ}" "${DIST}/zyvor-guest-agent.wxs"
  light -out "${DIST}/zyvor-vm-tools-${VERSION}.msi" "${WIX_OBJ}"
  echo "MSI: ${DIST}/zyvor-vm-tools-${VERSION}.msi"
else
  echo "WiX not available or agent binary missing — publishing zip scaffold."
  if [[ ! -f "${ZIP_PATH}" ]]; then
    (cd "${STAGING}" && zip -r "${ZIP_PATH}" .)
  fi
  echo "Zip: ${ZIP_PATH}"
  echo "For MSI: install WiX Toolset, ensure guestkitd.exe is in staging, re-run."
fi
