#!/usr/bin/env bash
# Copyright 2026 Zyvor
# SPDX-License-Identifier: Apache-2.0
# ============================================================================
# deploy-ui-remote.sh — Deploy GuestKit web UI with built-in HTTPS (no nginx)
# ============================================================================
# Ships deploy/ui to the remote host and runs serve-https.py under systemd.
# TLS is terminated by Python's ssl module (self-signed lab cert under tls/).
#
# Usage:
#   ./scripts/deploy-ui-remote.sh <host> [user] [password] [options]
#   ./scripts/deploy-ui-remote.sh 212.8.248.187 sus --port 27173
#   GUESTKIT_UI_PORT=27173 ./scripts/deploy-ui-remote.sh 212.8.248.187 sus
#   ./scripts/deploy-ui-remote.sh 212.8.248.187 sus --uninstall
#
# Options:
#   --port N      Host TCP port for HTTPS
#   --uninstall   Stop/remove the guestkit-ui service + files
#   --dry-run     Print what would happen; make no changes
#   --skip-smoke  Skip the smoke-ui-remote.sh step
#
# Port resolution (first match wins):
#   1. --port N
#   2. GUESTKIT_UI_PORT env
#   3. PORT from .deploy-ui-last (redeploy keeps the same port)
#   4. random in 18000–28999
# ============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
# shellcheck source=lib/deploy-ui-remote-common.sh
source "$SCRIPT_DIR/lib/deploy-ui-remote-common.sh"

info()  { guestkit_ui_info "$@"; }
warn()  { guestkit_ui_warn "$@"; }
error() { guestkit_ui_error "$@"; }
step()  { LAST_ACTION="$*"; deploy_ui_step_start "$*"; }

SERVICE_NAME=guestkit-ui
REMOTE_DIR=/opt/guestkit-ui
STATE_FILE=".deploy-ui-last"
OLD_CONTAINER=guestkit-ui

UNINSTALL_MODE=false
DRY_RUN=false
SKIP_SMOKE=false
PORT_FROM_CLI=""
POSITIONAL=()
while [ $# -gt 0 ]; do
    case "$1" in
        --port)
            [ $# -ge 2 ] || error "--port requires a value"
            PORT_FROM_CLI="$2"
            shift 2
            ;;
        --port=*)
            PORT_FROM_CLI="${1#*=}"
            shift
            ;;
        --uninstall)  UNINSTALL_MODE=true; shift ;;
        --dry-run)    DRY_RUN=true; shift ;;
        --skip-smoke) SKIP_SMOKE=true; shift ;;
        --help|-h)
            sed -n '2,36p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        --)
            shift
            POSITIONAL+=("$@")
            break
            ;;
        -*)
            error "Unknown option: $1 (see --help)"
            ;;
        *)
            POSITIONAL+=("$1")
            shift
            ;;
    esac
done

HOST="${POSITIONAL[0]:-${DEPLOY_HOST:-}}"
USER="${POSITIONAL[1]:-${DEPLOY_USER:-root}}"
PASS="${POSITIONAL[2]:-${DEPLOY_PASS:-}}"

guestkit_ui_parse_target HOST USER
LAST_PORT=""
STATE_PATH="$REPO_DIR/$STATE_FILE"
deploy_ui_deploy_state_file() { echo "$1/$STATE_FILE"; }

if [ -z "$HOST" ] && guestkit_ui_load_deploy_last "$REPO_DIR"; then
    info "Using $STATE_FILE → ${USER}@${HOST}"
    LAST_PORT="${PORT:-}"
elif [ -f "$STATE_PATH" ]; then
    LAST_PORT="$(awk -F= '/^PORT=/ {print $2; exit}' "$STATE_PATH")"
fi
[ -z "$HOST" ] && error "Usage: $0 <host> [user] [password] [options]  (see --help)"

if [ -n "$PORT_FROM_CLI" ]; then
    GUESTKIT_UI_PORT="$PORT_FROM_CLI"
elif [ -n "${GUESTKIT_UI_PORT:-}" ]; then
    :
elif [ -n "$LAST_PORT" ]; then
    GUESTKIT_UI_PORT="$LAST_PORT"
    info "Reusing port ${GUESTKIT_UI_PORT} from $STATE_FILE"
else
    GUESTKIT_UI_PORT=$((18000 + RANDOM % 11000))
    info "Selected random port ${GUESTKIT_UI_PORT}"
fi
case "$GUESTKIT_UI_PORT" in
    ''|*[!0-9]*) error "Invalid port: ${GUESTKIT_UI_PORT}" ;;
esac
if [ "$GUESTKIT_UI_PORT" -lt 1 ] || [ "$GUESTKIT_UI_PORT" -gt 65535 ]; then
    error "Port out of range: ${GUESTKIT_UI_PORT}"
fi

[ -f "$REPO_DIR/deploy/ui/serve-https.py" ] || error "deploy/ui/serve-https.py missing"
[ -f "$REPO_DIR/deploy/ui/index.html" ] || error "deploy/ui/index.html missing"
[ -f "$REPO_DIR/deploy/ui/demo-doctor.json" ] || error "deploy/ui/demo-doctor.json missing"
guestkit_ui_build_metadata "$REPO_DIR"
DEPLOY_UI_PORT="$GUESTKIT_UI_PORT"
DEPLOY_UI_SCHEME="https"

SUDO=""
[ "$USER" != "root" ] && SUDO="sudo"

DEPLOY_SSH_OPTS=(
    -o StrictHostKeyChecking=no
    -o ConnectTimeout=15
    -o ServerAliveInterval=15
    -o ServerAliveCountMax=8
)
DEPLOY_SSH_TTY_OPTS=()
[ "$USER" != "root" ] && DEPLOY_SSH_TTY_OPTS=(-tt)

if [ -n "$PASS" ] && ! command -v sshpass &>/dev/null; then
    error "sshpass required for password auth"
fi

_ssh() {
    local -a ssh_args=("${DEPLOY_SSH_OPTS[@]}" "${DEPLOY_SSH_TTY_OPTS[@]}")
    if [ -n "$PASS" ]; then
        SSHPASS="$PASS" sshpass -e ssh "${ssh_args[@]}" "${USER}@${HOST}" "$@"
    else
        ssh "${ssh_args[@]}" "${USER}@${HOST}" "$@"
    fi
}

_ssh_batch() {
    local -a ssh_args=("${DEPLOY_SSH_OPTS[@]}")
    if [ -n "$PASS" ]; then
        SSHPASS="$PASS" sshpass -e ssh "${ssh_args[@]}" "${USER}@${HOST}" "$@"
    else
        ssh "${ssh_args[@]}" "${USER}@${HOST}" "$@"
    fi
}

_scp() {
    local -a scp_args=("${DEPLOY_SSH_OPTS[@]}")
    if [ -n "$PASS" ]; then
        SSHPASS="$PASS" sshpass -e scp "${scp_args[@]}" "$@"
    else
        scp "${scp_args[@]}" "$@"
    fi
}

if $DRY_RUN; then
    deploy_ui_banner "${DEPLOY_UI_ICON_MAGIC} Dry run" "no changes will be made"
    deploy_ui_kv "🎯" "Target" "${USER}@${HOST}"
    deploy_ui_kv "🔐" "TLS" "built-in (serve-https.py)"
    deploy_ui_kv "🌐" "Port" "$GUESTKIT_UI_PORT"
    echo ""
    deploy_ui_note "Would: ship deploy/ui → systemd ${SERVICE_NAME} on :${GUESTKIT_UI_PORT} (HTTPS) → smoke"
    echo ""
    exit 0
fi

deploy_ui_banner "UI Remote Deploy" "${GUESTKIT_GIT_VERSION} (${GUESTKIT_GIT_COMMIT}) → ${USER}@${HOST}"
deploy_ui_kv "🎯" "Target" "${USER}@${HOST}"
deploy_ui_kv "🔐" "TLS" "built-in HTTPS (no nginx)"
deploy_ui_kv "🌐" "Port" "$GUESTKIT_UI_PORT"
echo ""

if $UNINSTALL_MODE; then
    deploy_ui_uninstall_banner
    step "Removing ${SERVICE_NAME} from ${HOST}"
    _ssh "
        $SUDO systemctl stop ${SERVICE_NAME} 2>/dev/null || true
        $SUDO systemctl disable ${SERVICE_NAME} 2>/dev/null || true
        $SUDO rm -f /etc/systemd/system/${SERVICE_NAME}.service
        $SUDO systemctl daemon-reload 2>/dev/null || true
        $SUDO docker rm -f ${OLD_CONTAINER} 2>/dev/null || true
        $SUDO rm -rf ${REMOTE_DIR}
    "
    info "guestkit-ui removed from ${HOST}"
    exit 0
fi

step "Shipping deploy/ui to ${HOST}"
BUILD_DIR="$(mktemp -d)"
trap 'rm -rf "$BUILD_DIR"' EXIT
UI_STAGE="$BUILD_DIR/ui"
mkdir -p "$UI_STAGE"
COPYFILE_DISABLE=1 tar -C "$REPO_DIR/deploy/ui" \
    --exclude='.DS_Store' --exclude='__pycache__' --exclude='tests' \
    --exclude='Dockerfile' --exclude='Dockerfile.ga' --exclude='nginx.conf' \
    -cf - . | tar -C "$UI_STAGE" -xf -
chmod +x "$UI_STAGE/serve-https.py"
COPYFILE_DISABLE=1 tar -C "$BUILD_DIR" -czf "$BUILD_DIR/ui.tgz" ui
_ssh "$SUDO mkdir -p ${REMOTE_DIR} && $SUDO chown ${USER}:${USER} ${REMOTE_DIR} 2>/dev/null || true"
_scp "$BUILD_DIR/ui.tgz" "${USER}@${HOST}:/tmp/guestkit-ui.tgz"
_ssh "
    set -euo pipefail
    mkdir -p ${REMOTE_DIR}
    tar -C ${REMOTE_DIR} -xzf /tmp/guestkit-ui.tgz
    rm -f /tmp/guestkit-ui.tgz
    chmod +x ${REMOTE_DIR}/ui/serve-https.py
"
info "UI sources synced to ${REMOTE_DIR}"

step "Installing systemd ${SERVICE_NAME} (built-in HTTPS)"
UNIT_TMP="$BUILD_DIR/${SERVICE_NAME}.service"
cat > "$UNIT_TMP" <<EOF
[Unit]
Description=GuestKit UI (built-in HTTPS)
After=network.target

[Service]
Type=simple
WorkingDirectory=${REMOTE_DIR}/ui
ExecStart=/usr/bin/python3 ${REMOTE_DIR}/ui/serve-https.py --port ${GUESTKIT_UI_PORT} --dir ${REMOTE_DIR}/ui
Restart=on-failure
RestartSec=3

[Install]
WantedBy=multi-user.target
EOF
_scp "$UNIT_TMP" "${USER}@${HOST}:/tmp/${SERVICE_NAME}.service"
_ssh "
    set -euo pipefail
    command -v python3 >/dev/null
    command -v openssl >/dev/null
    # Tear down legacy nginx Docker lab path if present
    $SUDO docker rm -f ${OLD_CONTAINER} 2>/dev/null || true
    $SUDO mv /tmp/${SERVICE_NAME}.service /etc/systemd/system/${SERVICE_NAME}.service
    $SUDO systemctl daemon-reload
    $SUDO systemctl enable ${SERVICE_NAME}
    $SUDO systemctl restart ${SERVICE_NAME}
    if command -v firewall-cmd &>/dev/null; then
        $SUDO firewall-cmd --permanent --add-port=${GUESTKIT_UI_PORT}/tcp 2>/dev/null || true
        $SUDO firewall-cmd --reload 2>/dev/null || true
    elif command -v ufw &>/dev/null; then
        $SUDO ufw allow ${GUESTKIT_UI_PORT}/tcp 2>/dev/null || true
    fi
    sleep 1
    $SUDO systemctl is-active ${SERVICE_NAME}
"
info "${SERVICE_NAME} listening on HTTPS port ${GUESTKIT_UI_PORT}"

step "Verifying deployment"
BASE_URL="https://${HOST}:${GUESTKIT_UI_PORT}"
DEPLOY_UI_SCHEME="https"
_ssh "curl -kfsS https://127.0.0.1:${GUESTKIT_UI_PORT}/ >/dev/null" \
    && info "UI OK (https://127.0.0.1:${GUESTKIT_UI_PORT}/, on-host)"

guestkit_ui_save_deploy_last "$REPO_DIR" "$HOST" "$USER" "ui"

deploy_ui_highlight "📋 Final checklist"
deploy_ui_checklist "service" "$(_ssh_batch "$SUDO systemctl is-active ${SERVICE_NAME}" | tr -d '\r')"
deploy_ui_checklist "https"   "$(_ssh_batch "curl -ksS -o /dev/null -w '%{http_code}' https://127.0.0.1:${GUESTKIT_UI_PORT}/" | tr -d '\r')"

guestkit_ui_print_success "$HOST" 0

if $SKIP_SMOKE; then
    info "Skipped smoke-ui-remote.sh (--skip-smoke)"
else
    step "Running scripts/smoke-ui-remote.sh against ${BASE_URL}"
    ( cd "$REPO_DIR" && GUESTKIT_UI_URL="$BASE_URL" ./scripts/smoke-ui-remote.sh )
fi
