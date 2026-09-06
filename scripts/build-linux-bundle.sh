#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Build the GuestKit Linux in-guest agent and assemble a self-contained
# bundle tarball — the Linux counterpart of scripts/build-windows-bundle.sh.
# For download and integration with Zeus OS / Veyron / Machina (drop the
# static binaries onto any Linux guest, or install via install.sh).
#
# Requirements (Linux host):
#   - rustup + (optional) x86_64-unknown-linux-musl target for a static build
#     (musl-tools). Without it, falls back to a dynamic glibc build.
#
# Outputs (under dist/linux/):
#   guestkitd guestkitctl guestkitd-exec           (raw binaries)
#   guestkit-agent-<version>-linux-amd64[-musl].tar.gz
#     (binaries + systemd units + agent-policy.yaml + install.sh)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="$(grep -m1 '^version' "${ROOT}/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/')"
DIST="${ROOT}/dist/linux"
MUSL_TARGET="x86_64-unknown-linux-musl"

echo "==> GuestKit Linux bundle v${VERSION}"
mkdir -p "${DIST}"

# 1. Build the Linux agent. Prefer a static musl build (runs on any distro,
#    no libc dependency — mirrors the self-contained Windows exes); fall back
#    to a dynamic glibc build if the musl target/toolchain is unavailable.
SUFFIX="linux-amd64"
BIN=""
if rustup target list --installed 2>/dev/null | grep -q "${MUSL_TARGET}" \
   && command -v musl-gcc >/dev/null 2>&1; then
  echo "==> Building static agent for ${MUSL_TARGET}"
  if ( cd "${ROOT}" && cargo build --release -p zyvor-guest-agent --target "${MUSL_TARGET}" ); then
    BIN="${ROOT}/target/${MUSL_TARGET}/release"
    SUFFIX="linux-amd64-musl"
  fi
fi
if [ -z "${BIN}" ]; then
  echo "==> Building glibc agent (musl unavailable)"
  ( cd "${ROOT}" && cargo build --release -p zyvor-guest-agent )
  BIN="${ROOT}/target/release"
fi

for exe in guestkitd guestkitctl guestkitd-exec; do
  cp "${BIN}/${exe}" "${DIST}/${exe}"
  strip "${DIST}/${exe}" 2>/dev/null || true
done
echo "    built: guestkitd guestkitctl guestkitd-exec (${SUFFIX})"

# 2. Assemble the bundle tree.
STAGE="${DIST}/stage"
rm -rf "${STAGE}"
mkdir -p "${STAGE}/bin" "${STAGE}/systemd" "${STAGE}/config"
cp "${DIST}/guestkitd" "${DIST}/guestkitctl" "${DIST}/guestkitd-exec" "${STAGE}/bin/"
cp "${ROOT}/templates/agent/guestkit-agent.service" \
   "${ROOT}/templates/agent/zyvor-guest-agent-exec.service" \
   "${STAGE}/systemd/" 2>/dev/null || true
cp "${ROOT}/templates/agent/agent-policy.yaml" "${STAGE}/config/" 2>/dev/null || true

# install.sh — install binaries + unit, create the service user, enable.
cat > "${STAGE}/install.sh" <<'SH'
#!/usr/bin/env bash
# GuestKit Linux Agent installer. Run as root.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
[ "$(id -u)" -eq 0 ] || { echo "run as root" >&2; exit 1; }

install -Dm755 "${HERE}/bin/guestkitd"      /usr/bin/guestkitd
install -Dm755 "${HERE}/bin/guestkitctl"    /usr/bin/guestkitctl
install -Dm755 "${HERE}/bin/guestkitd-exec" /usr/bin/guestkitd-exec
ln -sf /usr/bin/guestkitd /usr/bin/zyvor-guest-agent

id zyvor-agent >/dev/null 2>&1 || useradd --system --no-create-home --shell /usr/sbin/nologin zyvor-agent
install -Dm640 "${HERE}/config/agent-policy.yaml" /etc/guestkit/agent-policy.yaml 2>/dev/null || true

install -Dm644 "${HERE}/systemd/guestkit-agent.service" /etc/systemd/system/guestkit-agent.service
[ -f "${HERE}/systemd/zyvor-guest-agent-exec.service" ] && \
  install -Dm644 "${HERE}/systemd/zyvor-guest-agent-exec.service" /etc/systemd/system/zyvor-guest-agent-exec.service
systemctl daemon-reload
systemctl enable --now guestkit-agent.service
echo "GuestKit agent installed. Status: systemctl status guestkit-agent"
guestkitctl status 2>/dev/null || true
SH
chmod +x "${STAGE}/install.sh"

cat > "${STAGE}/README.txt" <<TXT
GuestKit Linux Agent v${VERSION} (${SUFFIX})

Install (as root):   ./install.sh
Manual:              copy bin/* to /usr/bin, install systemd/guestkit-agent.service
Self-test:           ./bin/guestkitd selftest /tmp/gk-selftest.json
Control CLI:         guestkitctl status | health | perf | services

The agent speaks framed JSON-RPC 2.0 over the QGA / dedicated virtio-serial
channel, AF_VSOCK, or a local unix socket (/run/guestkit/agent.sock).
TXT

# 3. Pack the tarball.
TARBALL="${DIST}/guestkit-agent-${VERSION}-${SUFFIX}.tar.gz"
( cd "${STAGE}" && tar czf "${TARBALL}" . )
echo "    built: $(basename "${TARBALL}") ($(du -h "${TARBALL}" | cut -f1))"

echo ""
echo "==> Done. Artifacts in ${DIST}:"
ls -1 "${DIST}"/guestkitd "${DIST}"/guestkitctl "${DIST}"/guestkitd-exec "${DIST}"/*.tar.gz 2>/dev/null | sed 's,.*/,    ,'
