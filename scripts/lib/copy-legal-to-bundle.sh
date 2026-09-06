#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Copy Zyvor legal pack into a customer bundle stage directory.
# Usage: copy-legal-to-bundle.sh <stage-dir> <repo-root>
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "${SCRIPT_DIR}/copy-zyvor-legal-to-bundle.sh" "${1:?}" "${2:?}"
