#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# ============================================================================
# package-binary-remote.sh — Build GuestKit on a remote Linux host and tarball it
# ============================================================================
# Usage:
#   ./scripts/package-binary-remote.sh <host> [user] [--fetch] [--reuse-build] [--skip-deps]
#
# See: docs/PACKAGE_BINARY_REMOTE.md
# ============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
# shellcheck source=lib/zyvor-company-accept.sh
source "${SCRIPT_DIR}/lib/zyvor-company-accept.sh"
require_zyvor_company_accept "${REPO_DIR}"

FETCH=false
REUSE_BUILD=false
SKIP_DEPS=false
POSITIONAL=()

for arg in "$@"; do
    case "$arg" in
        --fetch) FETCH=true ;;
        --reuse-build) REUSE_BUILD=true ;;
        --skip-deps) SKIP_DEPS=true ;;
        -h|--help)
            sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *) POSITIONAL+=("$arg") ;;
    esac
done

HOST="${POSITIONAL[0]:-${DEPLOY_HOST:-}}"
USER="${POSITIONAL[1]:-${DEPLOY_USER:-sus}}"
SSH_TIMEOUT="${DEPLOY_SSH_TIMEOUT:-20}"

[[ -n "${HOST}" ]] || { echo "Usage: $0 <host> [user] [--fetch] [--reuse-build]" >&2; exit 1; }

VERSION="${GUESTKIT_PACKAGE_VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' "${REPO_DIR}/Cargo.toml" | head -1)}"
VERSION="${VERSION:-0.3.3}"
ARCH="linux-amd64"
REMOTE="${USER}@${HOST}"
REMOTE_HOME=$(ssh -o BatchMode=yes -o ConnectTimeout="${SSH_TIMEOUT}" "${REMOTE}" 'echo "$HOME"')
BUILD_DIR="${REMOTE_HOME}/.deployment/guestkit-package"
OUT_DIR="${GUESTKIT_PACKAGE_DIR:-${REMOTE_HOME}/guestkit-dist}"
ARTIFACT="guestkit-${VERSION}-${ARCH}"
LOCAL_DIST="${REPO_DIR}/dist"

RSYNC_EXCLUDES=(
    --exclude='.git/'
    --exclude='target/'
    --exclude='.venv/'
    --exclude='venv/'
)

# shellcheck source=lib/package-remote-ui.sh
source "${SCRIPT_DIR}/lib/package-remote-ui.sh"

pkg_remote_banner "GuestKit" "${VERSION}" "${REMOTE}" "${ARCH}"

if [[ "${GUESTKIT_REMOTE_SKIP_SSH_CHECK:-}" != "1" ]]; then
    pkg_remote_phase "Preflight"
    ssh -o BatchMode=yes -o ConnectTimeout="${SSH_TIMEOUT}" -o StrictHostKeyChecking=accept-new "${REMOTE}" "true"
    pkg_ok "SSH ${REMOTE}"
fi

pkg_remote_phase "Sync source"
pkg_remote_kv "Build dir" "${BUILD_DIR}"
ssh "${REMOTE}" "mkdir -p '${BUILD_DIR}'"
rsync -az --delete "${RSYNC_EXCLUDES[@]}" \
    -e "ssh -o StrictHostKeyChecking=no -o ServerAliveInterval=15 -o ServerAliveCountMax=120" \
    "${REPO_DIR}/" "${REMOTE}:${BUILD_DIR}/"

if ! $SKIP_DEPS; then
    pkg_remote_phase "Build dependencies"
    ssh "${REMOTE}" bash -s <<REMOTE_DEPS
set -euo pipefail
cd '${BUILD_DIR}'
SUDO=""
[ "\$(id -u)" -ne 0 ] && command -v sudo &>/dev/null && SUDO=sudo
if command -v dnf &>/dev/null; then
  \$SUDO dnf install -y gcc make openssl-devel pkg-config curl git 2>&1 | tail -6 || true
elif command -v apt-get &>/dev/null; then
  \$SUDO apt-get update -qq
  \$SUDO apt-get install -y build-essential pkg-config curl git 2>&1 | tail -6 || true
fi
if ! command -v cargo &>/dev/null; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi
source "\$HOME/.cargo/env" 2>/dev/null || true
rustc --version
cargo --version
REMOTE_DEPS
fi

BUILD_NEEDED=true
if $REUSE_BUILD; then
    if ssh "${REMOTE}" "test -x '${BUILD_DIR}/target/release/guestkit'"; then
        BUILD_NEEDED=false
        pkg_ok "Reusing target/release/guestkit (--reuse-build)"
    fi
fi

if $BUILD_NEEDED; then
    pkg_remote_phase "Compile (cargo release)"
    pkg_info "Rust release build…"
    ssh "${REMOTE}" bash -s <<REMOTE_BUILD
set -euo pipefail
cd '${BUILD_DIR}'
source "\$HOME/.cargo/env" 2>/dev/null || true
bash scripts/build-linux-release.sh
test -x target/release/guestkit
REMOTE_BUILD
fi

pkg_remote_phase "Assemble customer bundle"
pkg_remote_kv "Output" "${OUT_DIR}/${ARTIFACT}"
ssh "${REMOTE}" bash -s <<REMOTE_PACK
set -euo pipefail
OUT_DIR='${OUT_DIR}'
BUILD_DIR='${BUILD_DIR}'
ARTIFACT='${ARTIFACT}'
VERSION='${VERSION}'

# shellcheck source=scripts/lib/package-guestkit-client-bundle.sh
source "\${BUILD_DIR}/scripts/lib/package-guestkit-client-bundle.sh"
STAGE="\${OUT_DIR}/\${ARTIFACT}"
package_guestkit_client_bundle "\${STAGE}" "\${BUILD_DIR}" "\${VERSION}"
package_guestkit_client_tarball "\${OUT_DIR}" "\${ARTIFACT}" "\${STAGE}"
ls -lh "\${OUT_DIR}/\${ARTIFACT}.tar.gz"
"\${STAGE}/guestkit" --version
REMOTE_PACK

TARBALL="${ARTIFACT}.tar.gz"
REMOTE_TARBALL="${OUT_DIR}/${TARBALL}"

if $FETCH; then
    pkg_remote_phase "Fetch to laptop"
    mkdir -p "${LOCAL_DIST}"
    scp -o StrictHostKeyChecking=no \
        "${REMOTE}:${REMOTE_TARBALL}" \
        "${REMOTE}:${OUT_DIR}/${TARBALL}.sha256" \
        "${LOCAL_DIST}/"
    (cd "${LOCAL_DIST}" && shasum -a 256 -c "${TARBALL}.sha256" 2>/dev/null || sha256sum -c "${TARBALL}.sha256") && pkg_ok "Checksum verified"
fi

pkg_remote_done "GuestKit" "${REMOTE}:${REMOTE_TARBALL}" "${REMOTE}:${OUT_DIR}/${TARBALL}.sha256"
