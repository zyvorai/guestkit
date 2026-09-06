#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Build the GuestKit Windows agent, MSI installer, and a bootable-media ISO
# that bundles everything — for download and integration with Zeus OS /
# Veyron / Machina (attach the ISO as a CD-ROM to a Windows VM).
#
# Requirements (Linux host):
#   - rustup + x86_64-pc-windows-gnu target + mingw-w64 (gcc-mingw-w64-x86-64)
#   - msitools `wixl` (apt install wixl)   — for the MSI
#   - genisoimage                          — for the ISO
#
# Outputs (under dist/windows/):
#   guestkitd.exe guestkitctl.exe guestkitd-exec.exe
#   guestkit-agent-<version>.msi
#   guestkit-agent-<version>.iso   (MSI + binaries + install.bat + agent-policy.yaml)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="$(grep -m1 '^version' "${ROOT}/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/')"
TARGET="x86_64-pc-windows-gnu"
DIST="${ROOT}/dist/windows"
STAGE="${DIST}/stage"
CDROOT="${DIST}/cd"

echo "==> GuestKit Windows bundle v${VERSION}"
mkdir -p "${DIST}" "${STAGE}" "${CDROOT}/gk"

# 1. Cross-compile the Windows agent.
echo "==> Cross-compiling agent for ${TARGET}"
rustup target add "${TARGET}" >/dev/null 2>&1 || true
( cd "${ROOT}" && cargo build --release -p zyvor-guest-agent --target "${TARGET}" )
BIN="${ROOT}/target/${TARGET}/release"
for exe in guestkitd guestkitctl guestkitd-exec; do
  cp "${BIN}/${exe}.exe" "${DIST}/${exe}.exe"
done
echo "    built: guestkitd.exe guestkitctl.exe guestkitd-exec.exe"

# 2. Build the MSI (wixl).
if command -v wixl >/dev/null 2>&1; then
  echo "==> Building MSI with wixl"
  cp "${DIST}/guestkitd.exe" "${DIST}/guestkitctl.exe" "${STAGE}/"
  cp "${ROOT}/templates/agent/agent-policy.yaml" "${STAGE}/agent-policy.yaml"
  cp "${ROOT}/packaging/vmtools/windows/guestkit-wixl.wxs" "${STAGE}/"
  MSI="${DIST}/guestkit-agent-${VERSION}.msi"
  ( cd "${STAGE}" && wixl -o "${MSI}" guestkit-wixl.wxs )
  echo "    built: $(basename "${MSI}")"
else
  echo "!!  wixl not found — skipping MSI (apt install wixl). Bundling binaries only."
  MSI=""
fi

# 3. Assemble the bundle ISO.
echo "==> Building ISO"
cp "${DIST}/guestkitd.exe" "${DIST}/guestkitctl.exe" "${CDROOT}/gk/"
cp "${ROOT}/templates/agent/agent-policy.yaml" "${CDROOT}/gk/"
[ -n "${MSI}" ] && cp "${MSI}" "${CDROOT}/gk/guestkit-agent.msi"

# install.bat — silent MSI install then a status file (run inside the guest).
cat > "${CDROOT}/gk/install.bat" <<'BAT'
@echo off
REM GuestKit Agent — install from this CD. Run as Administrator.
if exist "%~dp0guestkit-agent.msi" (
  msiexec /i "%~dp0guestkit-agent.msi" /qn /norestart /l*v "%TEMP%\guestkit-msi.log"
  echo MSI install exit %ERRORLEVEL%
) else (
  echo No MSI on media; copy binaries manually from %~dp0
)
sc query GuestKitAgent
BAT

# selftest.bat — run the agent probe battery to a file (validation).
cat > "${CDROOT}/gk/selftest.bat" <<'BAT'
@echo off
"%~dp0guestkitd.exe" selftest "%TEMP%\guestkit-selftest.json"
echo selftest written to %TEMP%\guestkit-selftest.json
BAT

cat > "${CDROOT}/autorun.inf" <<'INF'
[autorun]
label=GuestKit Agent
INF

ISO="${DIST}/guestkit-agent-${VERSION}.iso"
genisoimage -o "${ISO}" -J -R -V "GUESTKIT" "${CDROOT}" >/dev/null 2>&1
echo "    built: $(basename "${ISO}") ($(du -h "${ISO}" | cut -f1))"

echo ""
echo "==> Done. Artifacts in ${DIST}:"
ls -1 "${DIST}"/*.exe "${DIST}"/*.msi "${DIST}"/*.iso 2>/dev/null | sed 's,.*/,    ,'
