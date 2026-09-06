#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Require acceptance of ZYVOR-COMPANY-TERMS.md (GuestKit — does not replace Apache LICENSE).
# shellcheck shell=bash

zyvor_terms_file() {
    local root="${1:?}"
    echo "${root}/ZYVOR-COMPANY-TERMS.md"
}

zyvor_terms_hash() {
    local file="${1:?}"
    if command -v shasum &>/dev/null; then
        shasum -a 256 "$file" | awk '{print $1}'
    elif command -v sha256sum &>/dev/null; then
        sha256sum "$file" | awk '{print $1}'
    else
        wc -c <"$file" | tr -d ' '
    fi
}

zyvor_terms_record_path() {
    echo "${HOME}/.guestkit/zyvor-company-acceptance.json"
}

zyvor_terms_already_recorded() {
    local root="${1:?}"
    local terms_file hash record
    terms_file="$(zyvor_terms_file "$root")"
    hash="$(zyvor_terms_hash "$terms_file")"
    record="$(zyvor_terms_record_path)"
    [ -f "$record" ] || return 1
    command -v python3 &>/dev/null || return 1
    python3 - "$record" "$hash" <<'PY' >/dev/null 2>&1
import json, sys
path, want = sys.argv[1], sys.argv[2]
with open(path) as f:
    data = json.load(f)
sys.exit(0 if data.get("termsHash") == want and data.get("accepted") is True else 1)
PY
}

zyvor_terms_write_record() {
    local root="${1:?}"
    local actor="${2:-${USER:-unknown}}"
    local terms_file hash record ts
    terms_file="$(zyvor_terms_file "$root")"
    hash="$(zyvor_terms_hash "$terms_file")"
    mkdir -p "${HOME}/.guestkit"
    record="$(zyvor_terms_record_path)"
    ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    if command -v python3 &>/dev/null; then
        python3 - "$record" "$hash" "$actor" "$ts" <<'PY'
import json, sys
path, h, actor, ts = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
with open(path, "w") as f:
    json.dump({
        "accepted": True,
        "termsVersion": "1.0",
        "termsHash": h,
        "acceptedAt": ts,
        "acceptedBy": actor,
        "product": "GuestKit",
        "codeLicense": "Apache-2.0",
        "copyrightOwner": "ZyvorAI Labs Private Limited",
        "company": "ZyvorAI Labs Private Limited",
        "brand": "zyvor.dev",
        "contact": "info@zyvor.dev",
    }, f, indent=2)
    f.write("\n")
PY
    else
        printf '{"accepted":true,"termsHash":"%s","acceptedAt":"%s"}\n' "$hash" "$ts" >"$record"
    fi
}

zyvor_terms_show_summary() {
    local root="${1:?}"
    local terms_file
    terms_file="$(zyvor_terms_file "$root")"
    echo ""
    echo "═══════════════════════════════════════════════════════════════"
    echo "  GuestKit — Zyvor company terms (zyvor.dev)"
    echo "═══════════════════════════════════════════════════════════════"
    echo "  GuestKit SOURCE: Apache-2.0 — see LICENSE (ZyvorAI Labs Private Limited)"
    echo "  Zyvor DISTRIBUTION: ${terms_file}"
    echo ""
    sed -n '1,22p' "$terms_file" | sed 's/^/  /'
    echo "  ..."
    echo ""
    echo "  Type ACCEPT to agree to Zyvor company terms for this distribution."
    echo "  (Apache 2.0 still applies to GuestKit source code.)"
    echo "═══════════════════════════════════════════════════════════════"
    echo ""
}

require_zyvor_company_accept() {
    local root="${1:?}"
    local terms_file
    terms_file="$(zyvor_terms_file "$root")"
    if [ ! -f "$terms_file" ]; then
        echo "ERROR: Missing ${terms_file}" >&2
        exit 1
    fi

    if zyvor_terms_already_recorded "$root"; then
        return 0
    fi

    if [ "${GUESTKIT_ZYVOR_ACCEPT:-}" = "1" ]; then
        zyvor_terms_write_record "$root" "${GUESTKIT_ZYVOR_ACTOR:-${USER:-unknown}}"
        echo "✅ Zyvor company terms accepted (GUESTKIT_ZYVOR_ACCEPT=1)"
        return 0
    fi

    if [ ! -t 0 ] || [ ! -t 1 ]; then
        echo "ERROR: Zyvor company terms not accepted." >&2
        echo "  Read: ${terms_file} and LICENSE (Apache 2.0)" >&2
        echo "  Then run interactively and type ACCEPT, or:" >&2
        echo "    export GUESTKIT_ZYVOR_ACCEPT=1" >&2
        exit 1
    fi

    zyvor_terms_show_summary "$root"
    echo -n "Type ACCEPT for Zyvor company terms: "
    local reply
    read -r reply
    if [ "$reply" != "ACCEPT" ]; then
        echo "Terms not accepted. Aborted." >&2
        exit 1
    fi
    zyvor_terms_write_record "$root" "${USER:-unknown}"
    echo "✅ Accepted. Record: $(zyvor_terms_record_path)"
}
