#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# ─────────────────────────────────────────────────────────────────────────────
# test-feature-matrix.sh — compile+clippy every Cargo feature/combination and
# live-exercise the external-service features (ai, cloud).
#
# Depth: compile matrix (build + clippy per row, no offline test execution).
# External services: run live when a provider/creds are reachable, else SKIP.
#
# Run on a Linux build host that has the system deps:
#   libhivex-dev (registry-write), libsystemd-dev (journal-native),
#   qemu-img, rustup+clippy, python3 + maturin (python-bindings row).
#
# Live-ai env (optional):   OLLAMA_HOST=http://127.0.0.1:11434  (or *_API_KEY),
#                           GK_TEST_DISK=/path/to/inspectable.qcow2
# Live-cloud env (optional):GK_TEST_S3_URI / GK_TEST_AZURE_URI / GK_TEST_GCS_URI
#                           plus the matching aws/az/gsutil CLI + credentials.
#
# Exit code: non-zero if any compile row (build or clippy) FAILS. SKIPs (missing
# live provider/creds) never fail the run but are listed in the summary.
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_DIR" || { echo "cannot cd to $REPO_DIR" >&2; exit 1; }
# shellcheck disable=SC1090
source "$HOME/.cargo/env" 2>/dev/null || true

LOGDIR="$(mktemp -d "${TMPDIR:-/tmp}/gk-featmatrix.XXXXXX")"
echo "Feature matrix — repo: $REPO_DIR"
echo "Logs: $LOGDIR"
echo

NAMES=(); RESULTS=(); DURATIONS=(); NOTES=()
FAILED=0

_now() { date +%s; }

run_row() {   # run_row <name> <cmd...>
    local name="$1"; shift
    local log="$LOGDIR/${name//[^a-zA-Z0-9_.-]/_}.log"
    local start end dur
    start=$(_now)
    if "$@" >"$log" 2>&1; then
        end=$(_now); dur=$((end - start))
        NAMES+=("$name"); RESULTS+=("PASS"); DURATIONS+=("${dur}s"); NOTES+=("")
        printf '  \033[32mPASS\033[0m  %-30s %4ss\n' "$name" "$dur"
    else
        end=$(_now); dur=$((end - start))
        NAMES+=("$name"); RESULTS+=("FAIL"); DURATIONS+=("${dur}s"); NOTES+=("log: $log")
        FAILED=$((FAILED + 1))
        printf '  \033[31mFAIL\033[0m  %-30s %4ss\n' "$name" "$dur"
        echo "      ── tail of $log ──"
        tail -20 "$log" | sed 's/^/      /'
    fi
}

skip_row() {  # skip_row <name> <reason>
    NAMES+=("$1"); RESULTS+=("SKIP"); DURATIONS+=("-"); NOTES+=("$2")
    printf '  \033[33mSKIP\033[0m  %-30s %s\n' "$1" "$2"
}

# build + clippy for a feature-flag set: bc <name> [cargo flags...]
bc() {
    local name="$1"; shift
    run_row "build:$name"  cargo build "$@"
    run_row "clippy:$name" cargo clippy --all-targets "$@" -- -D warnings
}

echo "toolchain: $(rustc --version 2>/dev/null) / $(cargo --version 2>/dev/null)"
echo "clippy:    $(cargo clippy --version 2>/dev/null || echo 'MISSING — rustup component add clippy')"
echo

# ── A. Compile + clippy matrix ───────────────────────────────────────────────
echo "── A. compile + clippy matrix ──"
bc default
bc no-default    --no-default-features
bc guest-inspect --no-default-features --features guest-inspect
bc journal-native --no-default-features --features journal-native
bc registry-write --features registry-write
bc agent         --features agent
bc ai            --features ai
bc local-ai      --features local-ai
bc cloud-s3      --features cloud-s3
bc cloud-azure   --features cloud-azure
bc cloud-gcs     --features cloud-gcs
bc cloud         --features cloud
bc deploy-combo  --features "agent registry-write"
# all-features: build --lib only (pyo3 extension-module can't link the bin; matches CI)
run_row "build:all-features(lib)" cargo build --all-features --lib
run_row "clippy:all-features"     cargo clippy --all-targets --all-features -- -D warnings

# Confirm the registry-write build actually links libhivex (not just checks).
if [ -x target/debug/guestkit ]; then
    run_row "link:registry-write(libhivex)" bash -c \
        'cargo build --features registry-write --bin guestkit >/dev/null 2>&1 && ldd target/debug/guestkit | grep -qi hivex'
fi

# ── python-bindings via maturin (rlib can't produce a module) ────────────────
echo "── python-bindings (maturin) ──"
if command -v maturin >/dev/null 2>&1 || pip install --user -q maturin >/dev/null 2>&1; then
    export PATH="$HOME/.local/bin:$PATH"
    run_row "python-bindings(maturin build)" \
        env PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin build --release --features python-bindings --out "$LOGDIR/wheels"
else
    skip_row "python-bindings(maturin build)" "maturin not available and pip install failed"
fi

# ── B. Live external-service checks ──────────────────────────────────────────
echo "── B. live external services ──"

# ai / local-ai
ai_provider=""
if [ -n "${OLLAMA_HOST:-}" ] && curl -fsS "${OLLAMA_HOST%/}/api/tags" >/dev/null 2>&1; then
    ai_provider="ollama"
elif [ -n "${OPENAI_API_KEY:-}${ANTHROPIC_API_KEY:-}${XAI_API_KEY:-}" ]; then
    ai_provider="apikey"
fi
if [ -z "$ai_provider" ]; then
    skip_row "ai-live" "no provider (set OLLAMA_HOST reachable, or *_API_KEY)"
else
    ai_disk="${GK_TEST_DISK:-}"
    if [ -z "$ai_disk" ]; then
        # Fabricate a scratch disk so the AI/provider path is still exercised.
        ai_disk="$LOGDIR/ai-scratch.qcow2"
        cargo build --features ai --bin guestkit >/dev/null 2>&1 \
            && ./target/debug/guestkit create "$ai_disk" --size 256 --format qcow2 >/dev/null 2>&1 || true
    else
        cargo build --features ai --bin guestkit >/dev/null 2>&1 || true
    fi
    if [ -s "$ai_disk" ]; then
        run_row "ai-live($ai_provider)" ./target/debug/guestkit doctor "$ai_disk" --explain --ai
    else
        skip_row "ai-live" "provider=$ai_provider but no usable disk (set GK_TEST_DISK)"
    fi
fi

# cloud-s3 / azure / gcs — resolve+pull via `guestkit doctor <uri>`
for tuple in "s3:aws:GK_TEST_S3_URI:cloud-s3" \
             "azure:az:GK_TEST_AZURE_URI:cloud-azure" \
             "gcs:gsutil:GK_TEST_GCS_URI:cloud-gcs"; do
    IFS=: read -r prov cli urivar feat <<<"$tuple"
    uri="${!urivar:-}"
    if ! command -v "$cli" >/dev/null 2>&1; then
        skip_row "cloud-live:$prov" "no $cli CLI"; continue
    fi
    if [ -z "$uri" ]; then
        skip_row "cloud-live:$prov" "no $urivar set"; continue
    fi
    cargo build --features "$feat" --bin guestkit >/dev/null 2>&1 || true
    run_row "cloud-live:$prov" ./target/debug/guestkit doctor "$uri" --explain
done

# ── C. Summary ───────────────────────────────────────────────────────────────
echo
echo "════════════════════════ FEATURE MATRIX SUMMARY ════════════════════════"
pass=0; fail=0; skip=0
for i in "${!NAMES[@]}"; do
    printf '  %-6s %-32s %-5s %s\n' "${RESULTS[$i]}" "${NAMES[$i]}" "${DURATIONS[$i]}" "${NOTES[$i]}"
    case "${RESULTS[$i]}" in PASS) pass=$((pass+1));; FAIL) fail=$((fail+1));; SKIP) skip=$((skip+1));; esac
done
echo "─────────────────────────────────────────────────────────────────────────"
echo "  PASS=$pass  FAIL=$fail  SKIP=$skip   logs in $LOGDIR"
echo "═════════════════════════════════════════════════════════════════════════"

[ "$FAILED" -eq 0 ]
