#!/bin/sh
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Pre-snapshot hook: flush MySQL/MariaDB tables if service is active.
set -eu
if systemctl is-active --quiet mysql.service 2>/dev/null \
  || systemctl is-active --quiet mariadb.service 2>/dev/null; then
  if command -v mysqladmin >/dev/null 2>&1; then
    mysqladmin flush-tables >/dev/null 2>&1 || true
    echo "mysql flush-tables attempted"
    exit 0
  fi
fi
echo "mysql not active"
exit 0
