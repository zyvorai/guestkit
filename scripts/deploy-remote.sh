#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# ─────────────────────────────────────────────────────────────
# GuestKit — Remote deployment (SSH + rsync)
#
# Profiles:
#   default     Sync source → install deps → build on remote → verify
#   --quick     Rsync + remote build only (skip system deps / rustup)
#   --quick --build-local   Rsync pre-built Linux binary (build locally first)
#
# Auth: SSH keys (recommended). Password via sshpass is supported but deprecated.
#
# Post-deploy: remote scripts/selftest.sh
# ─────────────────────────────────────────────────────────────
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
# shellcheck source=lib/zyvor-company-accept.sh
source "${SCRIPT_DIR}/lib/zyvor-company-accept.sh"
require_zyvor_company_accept "${PROJECT_DIR}"
VERSION="1.1.0"
REMOTE_DIR=""
DEPLOY_PROFILE="full"
DEPLOY_LOG="${GUESTKIT_DEPLOY_LOG:-${HOME}/.guestkit/deploy-$(date +%Y%m%d-%H%M%S).log}"

QUICK_MODE=false
UNINSTALL=false
FLEET_FILE=""
KEY_AUTH=false
DRY_RUN=false
SKIP_SYNC=false
SKIP_VERIFY=false
BUILD_LOCAL=false
VERIFY_ONLY=false
PREFLIGHT_ONLY=false
VERBOSE=false
SSH_RETRIES="${GUESTKIT_SSH_RETRIES:-3}"
POSITIONAL=()

usage() {
    cat <<EOF
🔧 GuestKit remote deploy v${VERSION}

Usage:
  $0 <host> <user> [options]
  $0 user@host [options]
  $0 --fleet hosts.txt

Profiles:
  🏗️  (default)     Full remote build + system deps (qemu, nbd — not legacy appliance tooling)
  ⚡  --quick       Rsync + cargo build on remote (skip dep install)
  ⚡  --quick --build-local   Install locally built target/release/guestkit (Linux only)

Options:
  --help              Show this help
  --dry-run           Print steps without SSH/rsync/build
  --preflight-only    SSH + disk/sudo checks, then exit
  --verify-only       Run remote selftest only (no deploy)
  --skip-sync         Skip rsync (sources already on host)
  --skip-verify       Skip remote selftest
  --build-local       With --quick: use local release binary (Linux host required)
  --key               SSH key auth (clear password)
  --uninstall         Remove guestkit from host
  -v, --verbose       Verbose rsync

Environment:
  GUESTKIT_DEPLOY_LOG       Log file path
  GUESTKIT_SSH_RETRIES      SSH retry count (default: 3)
  DEPLOY_DIR                Override remote staging dir (default: ~/.deployments/guestkit)

Examples:
  $0 10.0.0.5 root --key
  $0 sus@10.0.0.5 --quick
  $0 10.0.0.5 root --build-local --quick
  $0 10.0.0.5 root --verify-only
  make deploy-remote H=10.0.0.5 U=root

Fleet file (one host per line):
  host user [password] [options]
  user@host root --quick
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        -h|--help)        usage; exit 0 ;;
        --quick)          QUICK_MODE=true; DEPLOY_PROFILE="quick"; shift ;;
        --uninstall)      UNINSTALL=true; shift ;;
        --key)            KEY_AUTH=true; shift ;;
        --dry-run)        DRY_RUN=true; shift ;;
        --skip-sync)      SKIP_SYNC=true; shift ;;
        --skip-verify)    SKIP_VERIFY=true; shift ;;
        --build-local)    BUILD_LOCAL=true; shift ;;
        --verify-only)    VERIFY_ONLY=true; shift ;;
        --preflight-only) PREFLIGHT_ONLY=true; shift ;;
        -v|--verbose)     VERBOSE=true; shift ;;
        --fleet)
            shift
            FLEET_FILE="${1:?--fleet requires a hosts file path}"
            shift
            ;;
        *)
            POSITIONAL+=("$1")
            shift
            ;;
    esac
done

TARGET_HOST="${POSITIONAL[0]:-}"
TARGET_USER="${POSITIONAL[1]:-root}"
TARGET_PASS="${POSITIONAL[2]:-}"

if [ "$KEY_AUTH" = true ]; then
    TARGET_PASS=""
fi

if [[ -n "${TARGET_HOST}" && "${TARGET_HOST}" == *"@"* ]]; then
    TARGET_USER="${TARGET_HOST%%@*}"
    TARGET_HOST="${TARGET_HOST#*@}"
fi

_use_color() { [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; }
if _use_color; then
    C_OK=$'\033[32m'; C_FAIL=$'\033[31m'; C_INFO=$'\033[36m'; C_WARN=$'\033[33m'
    C_DIM=$'\033[2m'; C_BOLD=$'\033[1m'; C_MAG=$'\033[35m'; C_CYAN=$'\033[96m'; C_RST=$'\033[0m'
else
    C_OK= C_FAIL= C_INFO= C_WARN= C_DIM= C_BOLD= C_MAG= C_CYAN= C_RST=
fi

_log_file() { mkdir -p "$(dirname "$DEPLOY_LOG")" 2>/dev/null || true; echo "[$(date -Iseconds)] $*" >>"$DEPLOY_LOG" 2>/dev/null || true; }
ok()   { echo "${C_OK}  ✅ $*${C_RST}"; _log_file "OK $*"; }
fail() { echo "${C_FAIL}  ❌ $*${C_RST}" >&2; _log_file "FAIL $*"; exit 1; }
info() { echo "${C_INFO}  💡 $*${C_RST}"; _log_file "INFO $*"; }
warn() { echo "${C_WARN}  ⚠️  $*${C_RST}"; _log_file "WARN $*"; }
dry()  { echo "${C_MAG}  👻 $*${C_RST}"; _log_file "DRY $*"; }

profile_emoji() {
    if [ "$UNINSTALL" = true ]; then echo "🗑️"; return; fi
    if [ "$VERIFY_ONLY" = true ]; then echo "🔬"; return; fi
    if [ "$PREFLIGHT_ONLY" = true ]; then echo "🔍"; return; fi
    [ "$QUICK_MODE" = true ] && echo "⚡" || echo "🏗️"
}

profile_label() {
    if [ "$UNINSTALL" = true ]; then echo "uninstall"; return; fi
    if [ "$VERIFY_ONLY" = true ]; then echo "verify-only"; return; fi
    if [ "$PREFLIGHT_ONLY" = true ]; then echo "preflight"; return; fi
    echo "${DEPLOY_PROFILE}"
}

print_banner() {
    local pe pl target
    pe=$(profile_emoji)
    pl=$(profile_label)
    target="${TARGET_USER}@${TARGET_HOST}"
    [ -z "${TARGET_HOST}" ] && target="(fleet mode)"
    echo ""
    echo "${C_CYAN}${C_BOLD}  ╔══════════════════════════════════════════════════════════╗${C_RST}"
    echo "${C_CYAN}${C_BOLD}  ║${C_RST}  ${pe}  🔧 ${C_BOLD}GuestKit${C_RST} Remote Deploy  ${C_DIM}v${VERSION}${C_RST}              ${C_CYAN}${C_BOLD}║${C_RST}"
    echo "${C_CYAN}${C_BOLD}  ║${C_RST}     🛰️  ${C_BOLD}${target}${C_RST}  ·  profile: ${pe} ${pl}                    ${C_CYAN}${C_BOLD}║${C_RST}"
    [ "$DRY_RUN" = true ] && echo "${C_MAG}${C_BOLD}  ║${C_RST}     👻  DRY-RUN — no remote changes                    ${C_MAG}${C_BOLD}║${C_RST}"
    [ -n "${FLEET_FILE}" ] && echo "${C_CYAN}${C_BOLD}  ║${C_RST}     🚢  Fleet: ${FLEET_FILE}                          ${C_CYAN}${C_BOLD}║${C_RST}"
    echo "${C_CYAN}${C_BOLD}  ╚══════════════════════════════════════════════════════════╝${C_RST}"
    echo ""
}

STEP_T0=0
STEP_IDX=0

step_begin() {
    STEP_IDX=$((STEP_IDX + 1))
    STEP_T0=$(date +%s)
    echo ""
    echo "${C_BOLD}${C_CYAN}  ┌─ Step ${STEP_IDX}: $*${C_RST}"
    echo "${C_DIM}  ────────────────────────────────────────────────────────────${C_RST}"
    _log_file "STEP ${STEP_IDX}: $*"
}

step_end() {
    echo "${C_DIM}  └─${C_RST} ${C_OK}✨ finished in $(( $(date +%s) - STEP_T0 ))s${C_RST}"
}

run_step() {
    step_begin "$1"; shift
    if [ "$DRY_RUN" = true ]; then dry "would run: $*"; step_end; return 0; fi
    "$@"; step_end
}

SSH_OPTS="-o StrictHostKeyChecking=accept-new -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR -o ConnectTimeout=15 -o ServerAliveInterval=30"
if [ -z "${TARGET_PASS}" ]; then
    SSH_OPTS+=" -o BatchMode=yes -o PreferredAuthentications=publickey"
fi

_ssh_once() {
    if [ -n "${TARGET_PASS}" ] && command -v sshpass &>/dev/null; then
        export SSHPASS="${TARGET_PASS}"
        sshpass -e ssh ${SSH_OPTS} "${TARGET_USER}@${TARGET_HOST}" "$@"
    else
        ssh ${SSH_OPTS} "${TARGET_USER}@${TARGET_HOST}" "$@"
    fi
}

_ssh() {
    local attempt=1 max="${SSH_RETRIES}"
    while [ "$attempt" -le "$max" ]; do
        if _ssh_once "$@"; then
            return 0
        fi
        attempt=$((attempt + 1))
        if [ "$attempt" -le "$max" ]; then
            local _d=$(( 2 * (attempt - 1) )); _d=$(( _d < 2 ? 2 : _d > 30 ? 30 : _d ))
            warn "SSH retry ${attempt}/${max}" && sleep "${_d}"
        fi
    done
    return 1
}

_rsync() {
    local opts="-az --delete"
    [ "$VERBOSE" = true ] && opts+=" --progress"
    if [ -n "${TARGET_PASS}" ] && command -v sshpass &>/dev/null; then
        export SSHPASS="${TARGET_PASS}"
        rsync ${opts} -e "sshpass -e ssh ${SSH_OPTS}" "$@"
    else
        rsync ${opts} -e "ssh ${SSH_OPTS}" "$@"
    fi
}

validate() {
    [ -n "${TARGET_HOST}" ] || { usage; exit 1; }
    [ -f "${PROJECT_DIR}/Cargo.toml" ] || fail "Not in guestkit repo: ${PROJECT_DIR}"
    if [ -n "${TARGET_PASS}" ]; then
        warn "Password auth is deprecated. Prefer: ssh-copy-id ${TARGET_USER}@${TARGET_HOST}"
        command -v sshpass &>/dev/null || fail "sshpass required for password auth (dnf/apt install sshpass)"
    fi
}

check_connectivity() {
    info "SSH → ${TARGET_USER}@${TARGET_HOST}  📝 ${DEPLOY_LOG}"
    if [ "$DRY_RUN" = true ]; then
        REMOTE_DIR="${DEPLOY_DIR:-${HOME}/.deployments/guestkit}"
        return 0
    fi
    _ssh "echo ok" &>/dev/null || fail "SSH failed — try: ssh-copy-id ${TARGET_USER}@${TARGET_HOST}"
    ok "SSH connected"
    local remote_home
    remote_home=$(_ssh "echo \$HOME" 2>/dev/null | tr -d '\r')
    remote_home="${remote_home:-/home/${TARGET_USER}}"
    REMOTE_DIR="${DEPLOY_DIR:-${remote_home}/.deployments/guestkit}"
    info "Remote path: ${REMOTE_DIR}"
}

preflight_remote() {
    info "Preflight on ${TARGET_HOST}..."
    if [ "$DRY_RUN" = true ]; then return 0; fi
    _ssh bash <<'REMOTE' || fail "Preflight failed"
set -e
echo "  host: $(hostname -f 2>/dev/null || hostname)"
echo "  os:   $(. /etc/os-release 2>/dev/null && echo "$PRETTY_NAME" || uname -s)"
echo "  arch: $(uname -m)"
echo "  mem:  $(free -h 2>/dev/null | awk '/^Mem:/{print $2}' || echo n/a)"
echo "  disk: $(df -h / 2>/dev/null | awk 'NR==2{print $4 " free on " $1}' || echo n/a)"
AVAIL=$(df -BG / 2>/dev/null | awk 'NR==2{gsub(/G/,"",$4); print $4}' || echo 99)
if [ "${AVAIL}" -lt 4 ] 2>/dev/null; then
    echo "  ⚠️  Less than 4G free on / — release build may fail"
fi
if [ "$(id -u)" -ne 0 ]; then
    if ! sudo -n true 2>/dev/null; then
        echo "  ❌ Non-root user needs passwordless sudo for modprobe/install"
        exit 1
    fi
    echo "  ✅ passwordless sudo"
else
    echo "  ✅ running as root"
fi
command -v curl >/dev/null && echo "  ✅ curl" || echo "  ⚠️  curl missing (needed for rustup)"
REMOTE
    ok "Preflight passed"
}

build_local_artifacts() {
    step_begin "Local build (release)"
    if [ "$DRY_RUN" = true ]; then
        dry "would run: cargo build --release"
        return 0
    fi
    if [ "$(uname -s)" != "Linux" ]; then
        fail "--build-local requires a Linux build host (same arch as remote). Use full deploy without --build-local."
    fi
    (cd "${PROJECT_DIR}" && bash scripts/build-linux-release.sh)
    [ -f "${PROJECT_DIR}/target/release/guestkit" ] || fail "target/release/guestkit missing"
    ok "Local binary ready"
    step_end
}

sync_files() {
    if [ "$SKIP_SYNC" = true ]; then
        info "Skipping rsync (--skip-sync)"
        return 0
    fi
    _ssh "mkdir -p '${REMOTE_DIR}'"
    local excludes=(
        --exclude '.git'
        --exclude 'target'
        --exclude 'proptest-regressions'
        --exclude 'presentations'
        --exclude '__pycache__'
        --exclude '*.pyc'
        --exclude '*.vmdk' --exclude '*.qcow2' --exclude '*.raw'
        --exclude '*.ova' --exclude '*.iso' --exclude '*.img'
        --exclude '*.vhd' --exclude '*.vhdx'
        --exclude 'test_results.txt'
    )
    _rsync "${excludes[@]}" "${PROJECT_DIR}/" "${TARGET_USER}@${TARGET_HOST}:${REMOTE_DIR}/"
    ok "Source synced to ${REMOTE_DIR}"
}

sync_binary_only() {
    local bin="${PROJECT_DIR}/target/release/guestkit"
    [ -f "$bin" ] || fail "Missing $bin — run with --build-local after building on Linux"
    _ssh "mkdir -p '${REMOTE_DIR}/bin'"
    _rsync "$bin" "${TARGET_USER}@${TARGET_HOST}:${REMOTE_DIR}/bin/guestkit"
    ok "Release binary synced"
}

install_system_deps() {
    _ssh bash <<'REMOTE'
set -euo pipefail
SUDO=""
[ "$(id -u)" -ne 0 ] && SUDO="sudo"

pkg_install() {
    # shellcheck disable=SC2068
    $SUDO "$@" || return 1
}

# Prefer apt-get on Debian/Ubuntu even if dnf is installed as an extra package
. /etc/os-release 2>/dev/null || true
_ID="${ID:-}" _ID_LIKE="${ID_LIKE:-}"
if [[ "$_ID" == "debian" || "$_ID" == "ubuntu" || "$_ID_LIKE" == *"debian"* || "$_ID_LIKE" == *"ubuntu"* ]]; then
    PKG=apt-get
    pkg_install apt-get update -qq
elif command -v dnf &>/dev/null; then
    PKG=dnf
elif command -v yum &>/dev/null; then
    PKG=yum
elif command -v apt-get &>/dev/null; then
    PKG=apt-get
    pkg_install apt-get update -qq
else
    echo "ERROR: unsupported package manager"
    exit 1
fi

if [ "$PKG" = "apt-get" ]; then
    pkg_install apt-get install -y -qq \
        qemu-utils nbd-client lvm2 parted e2fsprogs \
        build-essential pkg-config curl git \
        libsystemd-dev libhivex-dev
else
    # Core RPM packages (EL9: qemu-nbd is not a separate package — use qemu-img)
    pkg_install "$PKG" install -y \
        qemu-img nbd lvm2 parted e2fsprogs \
        gcc make openssl-devel pkg-config curl git \
        hivex-devel

    # qemu-nbd binary: bundled in qemu-img on EL9+, or separate package on older releases
    if ! command -v qemu-nbd &>/dev/null; then
        pkg_install "$PKG" install -y qemu-nbd 2>/dev/null \
            || pkg_install "$PKG" install -y qemu-kvm-tools 2>/dev/null \
            || true
        if ! command -v qemu-nbd &>/dev/null; then
            NBD_PKG=$($SUDO "$PKG" provides -q '*/qemu-nbd' 2>/dev/null | awk 'NF && $0 !~ /^Last/ {print $1; exit}')
            if [ -n "${NBD_PKG:-}" ]; then
                pkg_install "$PKG" install -y "$NBD_PKG" || true
            fi
        fi
    fi
    if command -v qemu-nbd &>/dev/null; then
        echo "  qemu-nbd: $(command -v qemu-nbd)"
    else
        echo "  ⚠️  qemu-nbd not in PATH (QCOW2/NBD mounts may be limited; qemu-img is installed)"
    fi
fi

$SUDO modprobe nbd max_part=16 2>/dev/null || true
echo 'nbd' | $SUDO tee /etc/modules-load.d/guestkit-nbd.conf >/dev/null 2>&1 || true
echo 'options nbd max_part=16' | $SUDO tee /etc/modprobe.d/guestkit-nbd.conf >/dev/null 2>&1 || true
echo "System dependencies installed"
REMOTE
}

ensure_rust_remote() {
    _ssh bash <<'REMOTE'
set -e
if command -v cargo &>/dev/null; then
    echo "Rust: $(rustc --version 2>/dev/null || true)"
    exit 0
fi
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
echo "Rust installed: $(rustc --version)"
REMOTE
}

build_install_remote() {
    _ssh env REMOTE_STAGING="${REMOTE_DIR}" bash <<'REMOTE'
set -e
SUDO=""
[ "$(id -u)" -ne 0 ] && SUDO="sudo"
source "$HOME/.cargo/env" 2>/dev/null || true
cd "${REMOTE_STAGING}"
# Include offline registry-write (libhivex FFI) when the dev headers are present.
if [ -z "${GUESTKIT_BUILD_FEATURES:-}" ]; then
    if pkg-config --exists hivex 2>/dev/null || ls /usr/include/hivex.h /usr/local/include/hivex.h &>/dev/null; then
        export GUESTKIT_BUILD_FEATURES="agent registry-write"
    fi
fi
bash scripts/build-linux-release.sh 2>&1 | tail -8
$SUDO install -m755 target/release/guestkit /usr/local/bin/guestkit
# Companion CLIs built from the same workspace (optional if a target was skipped).
for bin in virtctl-guestkit kubectl-guestkit guestkit-qemu; do
    if [ -x "target/release/${bin}" ]; then
        $SUDO install -m755 "target/release/${bin}" "/usr/local/bin/${bin}"
        echo "Installed: ${bin}"
    fi
done
# Runtime dirs for `guestkit vm` (definitions + QMP/pid sockets).
$SUDO mkdir -p /var/lib/guestkit/vms /run/guestkit/vms
if [ "$(id -u)" -ne 0 ]; then
    $SUDO chown -R "$(id -u):$(id -g)" /var/lib/guestkit /run/guestkit 2>/dev/null || true
fi
# Persist /run/guestkit across reboot (tmpfs).
OWNER_USER="$(id -un)"
OWNER_GROUP="$(id -gn)"
$SUDO tee /etc/tmpfiles.d/guestkit.conf >/dev/null <<EOF
d /run/guestkit 0755 ${OWNER_USER} ${OWNER_GROUP} -
d /run/guestkit/vms 0755 ${OWNER_USER} ${OWNER_GROUP} -
d /var/lib/guestkit/vms 0755 ${OWNER_USER} ${OWNER_GROUP} -
EOF
$SUDO systemd-tmpfiles --create /etc/tmpfiles.d/guestkit.conf 2>/dev/null || true
echo "Installed: $(guestkit --version 2>/dev/null || echo ok)"
REMOTE
}

install_binary_quick() {
    _ssh env REMOTE_STAGING="${REMOTE_DIR}" bash <<'REMOTE'
set -e
SUDO=""
[ "$(id -u)" -ne 0 ] && SUDO="sudo"
$SUDO install -m755 "${REMOTE_STAGING}/bin/guestkit" /usr/local/bin/guestkit
for bin in virtctl-guestkit kubectl-guestkit guestkit-qemu; do
    if [ -x "${REMOTE_STAGING}/bin/${bin}" ]; then
        $SUDO install -m755 "${REMOTE_STAGING}/bin/${bin}" "/usr/local/bin/${bin}"
    fi
done
$SUDO mkdir -p /var/lib/guestkit/vms /run/guestkit/vms
if [ "$(id -u)" -ne 0 ]; then
    $SUDO chown -R "$(id -u):$(id -g)" /var/lib/guestkit /run/guestkit 2>/dev/null || true
fi
OWNER_USER="$(id -un)"
OWNER_GROUP="$(id -gn)"
$SUDO tee /etc/tmpfiles.d/guestkit.conf >/dev/null <<EOF
d /run/guestkit 0755 ${OWNER_USER} ${OWNER_GROUP} -
d /run/guestkit/vms 0755 ${OWNER_USER} ${OWNER_GROUP} -
d /var/lib/guestkit/vms 0755 ${OWNER_USER} ${OWNER_GROUP} -
EOF
$SUDO systemd-tmpfiles --create /etc/tmpfiles.d/guestkit.conf 2>/dev/null || true
echo "Installed: $(guestkit --version 2>/dev/null || echo ok)"
REMOTE
}

verify_remote() {
    info "Running selftest on ${TARGET_HOST}..."
    if [ "$DRY_RUN" = true ]; then
        dry "would run: bash ${REMOTE_DIR}/scripts/selftest.sh"
        return 0
    fi
    # Single SSH attempt — do not retry when selftest exits non-zero
    if _ssh_once "bash '${REMOTE_DIR}/scripts/selftest.sh'"; then
        ok "selftest passed"
    else
        warn "selftest reported failures (guestkit is installed; see above)"
        return 0
    fi
}

do_uninstall() {
    _ssh env REMOTE_STAGING="${REMOTE_DIR}" bash <<'REMOTE'
set -e
SUDO=""
[ "$(id -u)" -ne 0 ] && SUDO="sudo"
$SUDO rm -f /usr/local/bin/guestkit /usr/local/bin/virtctl-guestkit \
    /usr/local/bin/kubectl-guestkit /usr/local/bin/guestkit-qemu
rm -rf "${REMOTE_STAGING}"
$SUDO rm -rf /var/cache/guestkit 2>/dev/null || true
echo "guestkit removed"
REMOTE
    ok "Uninstalled on ${TARGET_HOST}"
}

deploy_profile_full() {
    run_step "Sync sources" sync_files
    run_step "System dependencies" install_system_deps
    run_step "Rust toolchain" ensure_rust_remote
    run_step "Build and install" build_install_remote
}

deploy_profile_quick() {
    if [ "$BUILD_LOCAL" = true ]; then
        build_local_artifacts
        run_step "Sync binary" sync_binary_only
        run_step "Install binary" install_binary_quick
    else
        run_step "Sync sources" sync_files
        run_step "Build and install" build_install_remote
    fi
}

print_deployment_summary() {
    echo ""
    echo "${C_OK}${C_BOLD}  ╔══════════════════════════════════════════════════════════╗${C_RST}"
    echo "${C_OK}${C_BOLD}  ║${C_RST}  🎉  Deploy complete — ${TARGET_USER}@${TARGET_HOST}           ${C_OK}${C_BOLD}║${C_RST}"
    echo "${C_OK}${C_BOLD}  ╚══════════════════════════════════════════════════════════╝${C_RST}"
    echo ""
    echo "  📝  Log        ${DEPLOY_LOG}"
    echo "  📁  Remote     ${REMOTE_DIR}"
    echo ""
    echo "  🔗  ssh ${TARGET_USER}@${TARGET_HOST}"
    echo "  🔍  guestkit inspect /path/to/disk.qcow2"
    echo "  🎨  guestkit tui /path/to/disk.qcow2"
    echo "  🩺  bash ${REMOTE_DIR}/scripts/selftest.sh"
    echo ""
}

deploy_fleet() {
    local hosts_file="$1"
    [ -f "$hosts_file" ] || fail "Fleet file not found: $hosts_file"
    chmod 600 "$hosts_file" 2>/dev/null || true
    local count=0
    while IFS=' ' read -r host user pass opts; do
        [ -z "$host" ] && continue
        [[ "$host" =~ ^# ]] && continue
        count=$((count + 1))
        TARGET_HOST="$host"
        TARGET_USER="${user:-root}"
        TARGET_PASS="${pass:-}"
        if [[ "$host" == *"@"* ]]; then
            TARGET_USER="${host%%@*}"
            TARGET_HOST="${host#*@}"
        fi
        STEP_IDX=0
        print_banner
        check_connectivity
        preflight_remote
        if [[ "${opts:-}" == *"--uninstall"* ]]; then
            run_step "Uninstall" do_uninstall
        elif [[ "${opts:-}" == *"--quick"* ]]; then
            QUICK_MODE=true
            BUILD_LOCAL=false
            [[ "${opts:-}" == *"--build-local"* ]] && BUILD_LOCAL=true
            [ "$BUILD_LOCAL" = true ] && build_local_artifacts
            deploy_profile_quick
        else
            deploy_profile_full
        fi
        [ "$SKIP_VERIFY" != true ] && verify_remote
        print_deployment_summary
    done < "$hosts_file"
    ok "Fleet complete — ${count} host(s)"
}

main() {
    print_banner
    if [ -n "${FLEET_FILE}" ]; then
        validate
        deploy_fleet "${FLEET_FILE}"
        exit 0
    fi
    validate
    check_connectivity
    preflight_remote

    if [ "$PREFLIGHT_ONLY" = true ]; then
        ok "Preflight-only complete"
        exit 0
    fi

    if [ "$UNINSTALL" = true ]; then
        run_step "Uninstall guestkit" do_uninstall
        exit 0
    fi

    if [ "$VERIFY_ONLY" = true ]; then
        [ "$SKIP_VERIFY" != true ] && run_step "Verify" verify_remote
        print_deployment_summary
        exit 0
    fi

    if [ "$BUILD_LOCAL" = true ] && [ "$QUICK_MODE" != true ]; then
        warn "--build-local is intended with --quick; ignoring for full profile"
        BUILD_LOCAL=false
    fi

    case "${DEPLOY_PROFILE}" in
        quick) deploy_profile_quick ;;
        *)     deploy_profile_full ;;
    esac

    [ "$SKIP_VERIFY" != true ] && run_step "Verify (selftest)" verify_remote
    print_deployment_summary
}

main "$@"
