#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# CI / local recipe for cloud disk source unit tests + optional live pulls.
# Live pulls run only when GK_TEST_{S3,AZURE,GCS}_URI are set and CLIs exist.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "== cloud disk source unit tests =="
cargo test -p guestkit --lib storage::cloud_cache -- --nocapture
cargo test -p guestkit --features cloud-azure --lib storage::azure::tests -- --nocapture

echo "== feature matrix cloud rows (skip if no URI/CLI) =="
if [[ "${CI_CLOUD_LIVE:-0}" == "1" ]] || [[ -n "${GK_TEST_S3_URI:-}${GK_TEST_AZURE_URI:-}${GK_TEST_GCS_URI:-}" ]]; then
  ./scripts/test-feature-matrix.sh
else
  echo "skip live cloud pulls (set CI_CLOUD_LIVE=1 or GK_TEST_*_URI)"
fi

echo "cloud disk source recipe OK"
