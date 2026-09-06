#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Refresh vendored IronWolf shell CSS from ../ironwolf (manual review required).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SRC="${IRONWOLF_ROOT:-$(cd "$ROOT/../ironwolf" && pwd)}/web/dashboard/src/index.css"
OUT="$ROOT/deploy/ui/ironwolf-shell.css"
if [[ ! -f "$SRC" ]]; then
  echo "ERROR: IronWolf index.css not found at $SRC"
  exit 1
fi
{
  echo "/* GuestKit — IronWolf shell (auto-synced $(date -u +%Y-%m-%d); review before commit) */"
  sed -n '57,451p' "$SRC"
  sed -n '492,738p' "$SRC"
} > "$OUT"
echo "Wrote $OUT ($(wc -l < "$OUT") lines)"
