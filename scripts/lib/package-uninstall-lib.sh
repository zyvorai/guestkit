# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# shellcheck shell=bash
# Shared client uninstall helpers — sourced by uninstall.sh in the tarball.

_pu_msg() {
    if declare -f pkg_ok >/dev/null 2>&1; then
        pkg_ok "$@"
    else
        echo "  $*"
    fi
}

_pu_step() {
    if declare -f pkg_step >/dev/null 2>&1; then
        pkg_step "$@"
    else
        echo "► $*"
    fi
}

_pu_info() {
    if declare -f pkg_info >/dev/null 2>&1; then
        pkg_info "$@"
    else
        echo "  $*"
    fi
}

package_uninstall_usage() {
    cat <<'EOF'
Usage: ./uninstall.sh [options]

Remove this product's client install from the machine (stop processes,
delete config created by install.sh, optionally delete the whole bundle folder).

Options:
  --yes, -y          Do not ask for confirmation
  --keep-config      Leave *.env and system config files in place
  --remove-dir       Delete the entire extracted bundle directory when done
  --purge-deps       Show note about OS packages (not removed by default)
  -h, --help         This help

Examples:
  ./uninstall.sh --yes
  ./uninstall.sh --yes --remove-dir
EOF
}

package_uninstall_parse_args() {
    YES=false
    KEEP_CONFIG=false
    PURGE_DEPS=false
    REMOVE_DIR=false
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --yes|-y) YES=true ;;
            --keep-config) KEEP_CONFIG=true ;;
            --purge-deps) PURGE_DEPS=true ;;
            --remove-dir) REMOVE_DIR=true ;;
            -h|--help) package_uninstall_usage; exit 0 ;;
            *) echo "Unknown option: $1" >&2; package_uninstall_usage >&2; exit 1 ;;
        esac
        shift
    done
}

package_uninstall_confirm() {
    local msg="$1"
    [[ "${YES}" == true ]] && return 0
    read -r -p "${msg} [y/N] " ans
    [[ "${ans,,}" == "y" || "${ans,,}" == "yes" ]]
}

package_uninstall_stop_processes() {
    local root="$1"
    local b p
    for b in ${BINARIES:-}; do
        pkill -x "${b}" 2>/dev/null || true
        pkill -f "${root}/${b}" 2>/dev/null || true
        pkill -f "/${b}" 2>/dev/null || true
    done
    for b in ${BINARIES_SUBPATH:-}; do
        pkill -f "${b}" 2>/dev/null || true
    done
    if command -v fuser &>/dev/null; then
        for p in ${PORTS:-}; do
            fuser -k "${p}/tcp" 2>/dev/null || true
        done
    fi
    sleep 1
}

package_uninstall_remove_local_configs() {
    local root="$1"
    local f
    for f in ${LOCAL_CONFIGS:-}; do
        [[ -z "${f}" ]] && continue
        if [[ -f "${root}/${f}" ]]; then
            rm -f "${root}/${f}"
            _pu_msg "removed ${root}/${f}"
        fi
    done
    if [[ -f "${root}/.client-install-state" ]]; then
        rm -f "${root}/.client-install-state"
    fi
    return 0
}

package_uninstall_remove_system_paths() {
    local path
    for path in ${SYSTEM_PATHS:-}; do
        [[ -z "${path}" ]] && continue
        if [[ -e "${path}" ]]; then
            if [[ "${path}" == /etc/* || "${path}" == /var/* || "${path}" == /usr/* ]]; then
                sudo rm -rf "${path}" 2>/dev/null && _pu_msg "removed ${path}" || _pu_info "could not remove ${path} (sudo?)"
            else
                rm -rf "${path}" 2>/dev/null && _pu_msg "removed ${path}" || true
            fi
        fi
    done
}

package_uninstall_remove_systemd() {
    local u
    for u in ${SYSTEMD_UNITS:-}; do
        [[ -z "${u}" ]] && continue
        sudo systemctl stop "${u}" 2>/dev/null || true
        sudo systemctl disable "${u}" 2>/dev/null || true
        sudo rm -f "/etc/systemd/system/${u}" "/usr/lib/systemd/system/${u}" 2>/dev/null || true
        _pu_msg "stopped/disabled ${u}"
    done
    sudo systemctl daemon-reload 2>/dev/null || true
}

package_uninstall_purge_deps_note() {
    [[ "${PURGE_DEPS}" == true ]] || return 0
    echo ""
    echo "  OS packages (kubectl, libvirt, etc.) were NOT removed automatically."
    echo "  Remove manually if needed, or use your distro package manager."
    if [[ -x "${1}/install-client-deps.sh" ]]; then
        echo "  (install-client-deps.sh does not support --purge; deps are shared with other tools)"
    fi
}

package_uninstall_remove_bundle_dir() {
    local root="$1"
    local parent base tmp
    parent="$(dirname "${root}")"
    base="$(basename "${root}")"
    tmp="$(mktemp -t package-uninstall-XXXXXX.sh)"
    cat > "${tmp}" <<EOS
#!/usr/bin/env bash
sleep 1
rm -rf '${root}'
echo "Removed bundle directory: ${root}"
EOS
    chmod +x "${tmp}"
    _pu_info "scheduling removal of ${root}…"
    nohup "${tmp}" >/dev/null 2>&1 &
}

package_uninstall_main() {
    local product="${1:?}"
    local root="${2:?}"
    shift 2
    package_uninstall_parse_args "$@"

    ROOT="${root}"
    _PKG_SESSION_START=${SECONDS}

    if declare -f pkg_banner >/dev/null 2>&1; then
        pkg_counters_reset
        pkg_banner "${product} client uninstall" "Bundle: ${ROOT}"
    else
        echo ""
        echo "╔══════════════════════════════════════════════════════════╗"
        printf '║  %-54s ║\n' "${product} client uninstall"
        echo "╚══════════════════════════════════════════════════════════╝"
        echo ""
        echo "  Bundle: ${ROOT}"
        echo ""
    fi

    package_uninstall_confirm "Stop running processes and remove client install?" || {
        _pu_info "Cancelled."
        exit 0
    }

    _pu_step "Stopping processes"
    package_uninstall_stop_processes "${ROOT}"
    declare -f pkg_step_done >/dev/null 2>&1 && pkg_step_done

    if [[ "${KEEP_CONFIG}" != true ]]; then
        _pu_step "Removing configuration"
        package_uninstall_remove_local_configs "${ROOT}"
        package_uninstall_remove_system_paths
        package_uninstall_remove_systemd
        declare -f pkg_step_done >/dev/null 2>&1 && pkg_step_done
    else
        _pu_info "Keeping configuration (--keep-config)"
    fi

    package_uninstall_purge_deps_note "${ROOT}"

    if [[ "${REMOVE_DIR}" == true ]]; then
        package_uninstall_confirm "Delete entire bundle directory ${ROOT}?" || exit 0
        package_uninstall_remove_bundle_dir "${ROOT}"
        if declare -f pkg_summary >/dev/null 2>&1; then
            pkg_summary "Uninstall complete"
            pkg_next_steps "Bundle directory will be removed in a few seconds"
        else
            echo ""
            echo "  Bundle directory will be removed momentarily."
        fi
        exit 0
    fi

    if declare -f pkg_summary >/dev/null 2>&1; then
        pkg_summary "Uninstall complete"
        pkg_next_steps \
            "Binaries in this folder are unchanged" \
            "Delete folder: ./uninstall.sh --yes --remove-dir" \
            "Or: cd .. && rm -rf $(basename "${ROOT}")"
    else
        echo ""
        echo "══════════════════════════════════════════════════════════"
        echo "  Uninstall complete (binaries in this folder are unchanged)."
        echo "  To delete the whole package folder:"
        echo "    ./uninstall.sh --yes --remove-dir"
        echo "══════════════════════════════════════════════════════════"
    fi
    exit 0
}
