#!/bin/sh
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Pre-snapshot hook: flush PostgreSQL if local postgres service is active.
set -eu
if systemctl is-active --quiet postgresql.service 2>/dev/null \
  || systemctl is-active --quiet postgresql@*.service 2>/dev/null; then
  if command -v psql >/dev/null 2>&1; then
    psql -U postgres -c "SELECT pg_switch_wal();" >/dev/null 2>&1 || true
    echo "postgres wal switch attempted"
    exit 0
  fi
fi
echo "postgres not active"
exit 0
