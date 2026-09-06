#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Build guestkit release binary on Linux (used by deploy-remote.sh on remote hosts).
# Usage: ./scripts/build-linux-release.sh
# Env: GUESTKIT_BUILD_FEATURES (default: agent) — cargo --features
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FEATURES="${GUESTKIT_BUILD_FEATURES:-agent}"

prepare_cargo_target() {
    if [ -n "${TMPDIR:-}" ] && [ ! -d "$TMPDIR" ]; then
        mkdir -p "$TMPDIR"
    fi
    if [ -d "$ROOT/target/release" ] && [ ! -d "$ROOT/target/release/deps" ]; then
        echo "  Incomplete target/release — cargo clean"
        (cd "$ROOT" && cargo clean)
    fi
}

# Cargo walks up for [workspace]; e.g. guestkit at ~/.deployments/guestkit under machina workspace.
in_foreign_workspace() {
    local parent
    parent="$(dirname "$ROOT")"
    [ -f "${parent}/Cargo.toml" ] && grep -q '^\[workspace\]' "${parent}/Cargo.toml" 2>/dev/null
}

build_in_tree() {
    local tree="$1"
    cd "$tree"
    prepare_cargo_target
    echo "Building guestkit (release, features=${FEATURES}) in ${tree}..."
    if [ -n "$FEATURES" ]; then
        cargo build --release --features "$FEATURES"
    else
        cargo build --release
    fi
}

if in_foreign_workspace; then
    echo "  Parent workspace detected — isolated build"
    ISOLATED="$(mktemp -d)"
    trap 'rm -rf "$ISOLATED"' EXIT
    rsync -a --delete \
        --exclude target --exclude .git --exclude 'proptest-regressions' \
        "$ROOT/" "$ISOLATED/"
    build_in_tree "$ISOLATED"
    mkdir -p "$ROOT/target/release"
    cp -f "$ISOLATED/target/release/guestkit" "$ROOT/target/release/guestkit"
else
    build_in_tree "$ROOT"
fi

echo "  ✅ $("$ROOT/target/release/guestkit" --version 2>/dev/null || echo built)"
