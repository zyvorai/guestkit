#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# ============================================================================
# package-binary-release.sh — Build GuestKit and assemble customer tarball
# ============================================================================
# Used locally and by .github/workflows/release.yml (same bundle as remote pack).
#
# Usage:
#   ./scripts/package-binary-release.sh [--build] [--target TRIPLE] [--out-dir DIR]
#
# Environment:
#   GUESTKIT_PACKAGE_VERSION   Override version (default: Cargo.toml)
#   CARGO_TARGET               Rust target triple (same as --target)
# ============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

DO_BUILD=false
TARGET="${CARGO_TARGET:-}"
OUT_DIR="${GUESTKIT_PACKAGE_DIR:-${REPO_DIR}/dist}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --build) DO_BUILD=true; shift ;;
        --target)
            TARGET="${2:?--target requires a triple}"
            shift 2
            ;;
        --out-dir)
            OUT_DIR="${2:?--out-dir requires a path}"
            shift 2
            ;;
        -h|--help)
            sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
done

VERSION="${GUESTKIT_PACKAGE_VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' "${REPO_DIR}/Cargo.toml" | head -1)}"
VERSION="${VERSION:-0.3.3}"

case "${TARGET:-x86_64-unknown-linux-gnu}" in
    x86_64-unknown-linux-gnu|"") ARCH_SUFFIX="linux-amd64" ;;
    x86_64-unknown-linux-musl) ARCH_SUFFIX="linux-amd64-musl" ;;
    aarch64-unknown-linux-gnu) ARCH_SUFFIX="linux-arm64" ;;
    *)
        echo "Unsupported target for customer bundle: ${TARGET}" >&2
        exit 1
        ;;
esac

ARTIFACT="guestkit-${VERSION}-${ARCH_SUFFIX}"
TARGET_DIR="${REPO_DIR}/target"
if [[ -n "${TARGET}" ]]; then
    TARGET_DIR="${TARGET_DIR}/${TARGET}"
fi
BINARY="${GUESTKIT_BINARY:-${TARGET_DIR}/release/guestkit}"

if $DO_BUILD; then
    echo "Building guestkit (release)..."
    cd "${REPO_DIR}"
    if [[ -n "${TARGET}" ]]; then
        if [[ "${TARGET}" == *musl* ]]; then
            # Static musl build: minimal, no dynamic C deps (no libhivex/libsystemd).
            cargo build --release --target "${TARGET}" --bin guestkit \
                --no-default-features --features guest-inspect
        else
            # glibc build: enable offline registry writes when libhivex headers
            # are present (adds a runtime dependency on libhivex0).
            EXTRA_FEATURES=""
            if pkg-config --exists hivex 2>/dev/null \
                || ls /usr/include/hivex.h /usr/local/include/hivex.h &>/dev/null; then
                EXTRA_FEATURES="--features registry-write"
                echo "  libhivex found — enabling registry-write"
            fi
            cargo build --release --target "${TARGET}" --bin guestkit ${EXTRA_FEATURES}
        fi
    else
        bash scripts/build-linux-release.sh
    fi
fi

if [[ ! -x "${BINARY}" ]]; then
    echo "Missing release binary: ${BINARY} (pass --build or build first)" >&2
    exit 1
fi

# shellcheck source=lib/package-guestkit-client-bundle.sh
source "${SCRIPT_DIR}/lib/package-guestkit-client-bundle.sh"

STAGE="${OUT_DIR}/${ARTIFACT}"
export GUESTKIT_BINARY="${BINARY}"

echo "Assemble customer bundle → ${OUT_DIR}/${ARTIFACT}.tar.gz"
package_guestkit_client_bundle "${STAGE}" "${REPO_DIR}" "${VERSION}"
package_guestkit_client_tarball "${OUT_DIR}" "${ARTIFACT}" "${STAGE}"

"${BINARY}" --version
ls -lh "${OUT_DIR}/${ARTIFACT}.tar.gz"
echo "Done: ${OUT_DIR}/${ARTIFACT}.tar.gz"
