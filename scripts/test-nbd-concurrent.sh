#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
#
# Concurrent NBD allocate+connect regression (fluxvm#104 / guestkit flock).
#
# Always runs the flock unit test (no qemu-nbd needed). With a working NBD
# stack, also runs the ignored live connect test (20 connects @ concurrency 4).
#
# Usage:
#   ./scripts/test-nbd-concurrent.sh           # unit + live if nbd works
#   ./scripts/test-nbd-concurrent.sh --unit    # flock unit only
#   ./scripts/test-nbd-concurrent.sh --live    # live connect only (fail if nbd broken)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

MODE=all
case "${1:-}" in
  --unit) MODE=unit ;;
  --live) MODE=live ;;
  -h|--help)
    sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'
    exit 0
    ;;
  "") ;;
  *) echo "unknown option: $1" >&2; exit 64 ;;
esac

echo "==> NBD alloc lock unit (no qemu-nbd)"
cargo test --lib disk::nbd::tests::test_nbd_alloc_lock_serializes -- --exact --nocapture

if [[ "$MODE" == "unit" ]]; then
  echo "NBD_CONCURRENCY_UNIT=PASS"
  exit 0
fi

prepare_nbd() {
  if ! command -v qemu-nbd >/dev/null || ! command -v qemu-img >/dev/null; then
    echo "qemu-utils missing" >&2
    return 1
  fi
  if [[ "$(uname -s)" != "Linux" ]]; then
    echo "NBD live test requires Linux" >&2
    return 1
  fi
  sudo modprobe nbd max_part=16 >/dev/null 2>&1 || true
  if [[ ! -e /dev/nbd0 ]]; then
    echo "/dev/nbd0 not present after modprobe" >&2
    return 1
  fi
  # Same permission fix as ci.yml — wait_for_device opens the node without sudo.
  sudo chmod 666 /dev/nbd[0-9]* 2>/dev/null || true
  return 0
}

if ! prepare_nbd; then
  if [[ "$MODE" == "live" ]]; then
    echo "NBD_CONCURRENCY_LIVE=FAIL (nbd unavailable)" >&2
    exit 1
  fi
  echo "NBD live stack unavailable — skipping connect stress (use self-hosted nbd runner)"
  echo "NBD_CONCURRENCY_UNIT=PASS"
  echo "NBD_CONCURRENCY_LIVE=SKIP"
  exit 0
fi

echo "==> Concurrent NBD allocate+connect (20 @ concurrency 4)"
cargo test --lib disk::nbd::tests::test_concurrent_nbd_allocate_connect -- --exact --ignored --nocapture

echo "NBD_CONCURRENCY_UNIT=PASS"
echo "NBD_CONCURRENCY_LIVE=PASS"
