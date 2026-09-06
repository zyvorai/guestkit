#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# User install: default web/API login admin / Admin@321 (API key Admin@321).
# Sourced from package-ui.sh (bundled as .package-lib/package-auth-bootstrap.sh).
set -euo pipefail

pkg_env_set_var() {
    local env_file="$1" key="$2" value="$3"
    local tmp
    tmp=$(mktemp)
    if [[ -f "${env_file}" ]]; then
        grep -v "^${key}=" "${env_file}" > "${tmp}" 2>/dev/null || true
    else
        : > "${tmp}"
    fi
    echo "${key}=${value}" >> "${tmp}"
    mv "${tmp}" "${env_file}"
}

pkg_env_ensure_var() {
    local env_file="$1" key="$2" default_val="$3"
    [[ -f "${env_file}" ]] || touch "${env_file}"
    if grep -q "^${key}=" "${env_file}" 2>/dev/null; then
        local cur
        cur=$(grep "^${key}=" "${env_file}" | tail -1 | cut -d= -f2- | tr -d '\r')
        cur="${cur#\"}" cur="${cur%\"}"
        [[ -n "${cur}" && "${cur}" != "/path/to/"* && "${cur}" != "change-me"* && "${cur}" != "your-"* ]] && return 0
    fi
    pkg_env_set_var "${env_file}" "${key}" "${default_val}"
}

pkg_env_ensure_jwt_secret() {
    local env_file="$1" var="${2:-JWT_SECRET}"
    local js len
    js=""
    if grep -q "^${var}=" "${env_file}" 2>/dev/null; then
        js=$(grep "^${var}=" "${env_file}" | tail -1 | cut -d= -f2- | tr -d '\r')
        js="${js#\"}" js="${js%\"}"
    fi
    len=${#js}
    if [[ "${len}" -lt 32 ]]; then
        js=$(openssl rand -hex 32 2>/dev/null || head -c 64 /dev/urandom | od -An -tx1 | tr -d ' \n')
        pkg_env_set_var "${env_file}" "${var}" "${js}"
        pkg_ok "${var} written to ${env_file}"
    fi
}

pkg_ragnarok_admin_hash() {
    local pw="${PW:-Admin@321}"
    if command -v htpasswd >/dev/null 2>&1; then
        htpasswd -nbBC 12 '' "${pw}" 2>/dev/null | cut -d: -f2-
        return 0
    fi
    if command -v python3 >/dev/null 2>&1; then
        PW="${pw}" python3 - <<'PY' 2>/dev/null || true
import os, sys
pw = os.environ.get("PW", "Admin@321").encode()
try:
    import bcrypt
    print(bcrypt.hashpw(pw, bcrypt.gensalt(rounds=12)).decode())
except Exception:
    sys.exit(1)
PY
        return 0
    fi
    return 1
}

# Set API key / password / JWT per product env file (admin / Admin@321).
pkg_env_bootstrap_auth_for_file() {
    local env_file="$1"
    local root="${PKG_INSTALL_ROOT:-.}"
    local base="${env_file##*/}"
    base="${base%.env}"

    [[ -f "${env_file}" ]] || touch "${env_file}"

    case "${base}" in
        packetwolf)
            pkg_env_ensure_var "${env_file}" "PACKETWOLF_HOST" "0.0.0.0"
            pkg_env_ensure_var "${env_file}" "PACKETWOLF_PORT" "9191"
            pkg_env_ensure_var "${env_file}" "PACKETWOLF_ADMIN_API_KEY" "Admin@321"
            pkg_env_ensure_var "${env_file}" "PACKETWOLF_ADMIN_PASSWORD" "Admin@321"
            pkg_env_ensure_jwt_secret "${env_file}" "JWT_SECRET"
            pkg_env_ensure_var "${env_file}" "UI_DIST_DIR" "${root}/ui"
            ;;
        vmrogue)
            pkg_env_ensure_var "${env_file}" "VMROGUE_API_KEY" "Admin@321"
            ;;
        forge)
            pkg_env_ensure_var "${env_file}" "FORGE_API_KEY" "Admin@321"
            pkg_env_ensure_var "${env_file}" "FORGE_API_USER" "admin"
            ;;
        v9s)
            pkg_env_ensure_var "${env_file}" "V9S_API_KEY" "Admin@321"
            pkg_env_ensure_var "${env_file}" "V9S_LOCAL_ADMIN_PASSWORD" "Admin@321"
            ;;
        ironwolf)
            pkg_env_ensure_var "${env_file}" "IRONWOLF_API_KEY" "Admin@321"
            ;;
        aether)
            pkg_env_ensure_var "${env_file}" "AETHER_API_KEY" "Admin@321"
            ;;
        ragnarok)
            pkg_env_ensure_jwt_secret "${env_file}" "JWT_SECRET"
            local hash
            hash=$(PW=Admin@321 pkg_ragnarok_admin_hash) || hash=""
            if [[ -n "${hash}" ]]; then
                pkg_env_set_var "${env_file}" "RAGNAROK_ADMIN_PASSWORD_HASH" "${hash}"
                pkg_ok "RAGNAROK_ADMIN_PASSWORD_HASH set (login admin / Admin@321)"
            else
                pkg_warn "Install htpasswd or python3+bcrypt to seed admin — see backend docs"
            fi
            ;;
        hypersdk)
            pkg_env_ensure_var "${env_file}" "HYPERSDK_API_KEY" "Admin@321"
            ;;
        hyper2kvm)
            pkg_env_ensure_var "${env_file}" "HYPER2KVM_API_KEY" "Admin@321"
            ;;
        *)
            if [[ -n "${PKG_AUTH_API_KEY_VAR:-}" ]]; then
                pkg_env_ensure_var "${env_file}" "${PKG_AUTH_API_KEY_VAR}" "Admin@321"
            fi
            if [[ -n "${PKG_AUTH_PASSWORD_VAR:-}" ]]; then
                pkg_env_ensure_var "${env_file}" "${PKG_AUTH_PASSWORD_VAR}" "Admin@321"
            fi
            if [[ -n "${PKG_AUTH_JWT_VAR:-}" ]]; then
                pkg_env_ensure_jwt_secret "${env_file}" "${PKG_AUTH_JWT_VAR}"
            fi
            if [[ -n "${PKG_AUTH_USER_VAR:-}" ]]; then
                pkg_env_ensure_var "${env_file}" "${PKG_AUTH_USER_VAR}" "admin"
            fi
            [[ -n "${PKG_AUTH_API_KEY_VAR:-}${PKG_AUTH_PASSWORD_VAR:-}" ]] || return 0
            ;;
    esac

    pkg_ok "Web login: username admin · password Admin@321 (API key Admin@321 where applicable)"
}
