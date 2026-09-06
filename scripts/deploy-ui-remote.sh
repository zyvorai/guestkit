#!/usr/bin/env bash
# Copyright 2026 Zyvor
# SPDX-License-Identifier: Apache-2.0
# ============================================================================
# deploy-ui-remote.sh — Deploy GuestKit web UI (nginx) to a remote host via Docker
# ============================================================================
# Ships deploy/ui, builds guestkit-ui:zyvor-ga on the remote host, runs a
# container published on a configurable TCP port.
#
# Usage:
#   ./scripts/deploy-ui-remote.sh <host> [user] [password] [options]
#   ./scripts/deploy-ui-remote.sh 212.8.248.187 sus --port 27173
#   GUESTKIT_UI_PORT=27173 ./scripts/deploy-ui-remote.sh 212.8.248.187 sus
#   ./scripts/deploy-ui-remote.sh 212.8.248.187 sus --uninstall
#
# Options:
#   --port N      Host TCP port mapped to container :80
#   --uninstall   Stop/remove the guestkit-ui container
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

CONTAINER_NAME=guestkit-ui
IMAGE_NAME=guestkit-ui:zyvor-ga
STATE_FILE=".deploy-ui-last"

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
# Temporarily point deploy-ui state helpers at .deploy-ui-last
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

[ -f "$REPO_DIR/deploy/ui/Dockerfile" ] || error "deploy/ui/Dockerfile missing"
[ -f "$REPO_DIR/deploy/ui/zyvor-ux.js" ] || error "zyvor-ux.js missing — run the Zyvor orange GA apply first"
guestkit_ui_build_metadata "$REPO_DIR"
DEPLOY_UI_PORT="$GUESTKIT_UI_PORT"

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
    deploy_ui_kv "📦" "Image" "$IMAGE_NAME"
    deploy_ui_kv "🌐" "Port" "$GUESTKIT_UI_PORT"
    echo ""
    deploy_ui_note "Would: ship deploy/ui → docker build → run ${CONTAINER_NAME} -p ${GUESTKIT_UI_PORT}:80 → smoke"
    echo ""
    exit 0
fi

deploy_ui_banner "UI Remote Deploy" "${GUESTKIT_GIT_VERSION} (${GUESTKIT_GIT_COMMIT}) → ${USER}@${HOST}"
deploy_ui_kv "🎯" "Target" "${USER}@${HOST}"
deploy_ui_kv "🌐" "Port" "$GUESTKIT_UI_PORT"
echo ""

if $UNINSTALL_MODE; then
    deploy_ui_uninstall_banner
    step "Removing ${CONTAINER_NAME} from ${HOST}"
    _ssh "
        $SUDO docker rm -f $CONTAINER_NAME 2>/dev/null || true
        $SUDO rm -rf /opt/guestkit-ui
    "
    info "guestkit-ui removed from ${HOST}"
    exit 0
fi

step "Shipping deploy/ui to ${HOST}"
BUILD_DIR="$(mktemp -d)"
trap 'rm -rf "$BUILD_DIR"' EXIT
UI_STAGE="$BUILD_DIR/ui"
mkdir -p "$UI_STAGE"
# Prefer rsync-like copy of UI tree without Apple xattrs noise
COPYFILE_DISABLE=1 tar -C "$REPO_DIR/deploy/ui" \
    --exclude='.DS_Store' --exclude='__pycache__' --exclude='tests' \
    -cf - . | tar -C "$UI_STAGE" -xf -
# Standalone static nginx — no zyvor-api upstream required for lab UI deploys.
cat > "$UI_STAGE/nginx.conf" <<'NGINX'
server {
    listen 80;
    server_name _;
    root /usr/share/nginx/html;
    index index.html;

    location / {
        try_files $uri $uri/ /index.html;
    }

    location ~* \.(js|css)$ {
        add_header Cache-Control "no-cache, must-revalidate";
        try_files $uri =404;
    }
}
NGINX
COPYFILE_DISABLE=1 tar -C "$BUILD_DIR" -czf "$BUILD_DIR/ui.tgz" ui
_ssh "$SUDO mkdir -p /opt/guestkit-ui && $SUDO chown ${USER}:${USER} /opt/guestkit-ui 2>/dev/null || true"
_scp "$BUILD_DIR/ui.tgz" "${USER}@${HOST}:/tmp/guestkit-ui.tgz"
_ssh "
    set -euo pipefail
    mkdir -p /opt/guestkit-ui
    tar -C /opt/guestkit-ui -xzf /tmp/guestkit-ui.tgz
    rm -f /tmp/guestkit-ui.tgz
"
info "UI sources synced to /opt/guestkit-ui"

step "Building and starting ${CONTAINER_NAME}"
_ssh "
    set -euo pipefail
    cd /opt/guestkit-ui/ui
    $SUDO docker build -t $IMAGE_NAME -f Dockerfile .
    $SUDO docker rm -f $CONTAINER_NAME 2>/dev/null || true
    $SUDO docker run -d --name $CONTAINER_NAME --restart unless-stopped \
        -p ${GUESTKIT_UI_PORT}:80 $IMAGE_NAME
    if command -v firewall-cmd &>/dev/null; then
        $SUDO firewall-cmd --permanent --add-port=${GUESTKIT_UI_PORT}/tcp 2>/dev/null || true
        $SUDO firewall-cmd --reload 2>/dev/null || true
    elif command -v ufw &>/dev/null; then
        $SUDO ufw allow ${GUESTKIT_UI_PORT}/tcp 2>/dev/null || true
    fi
    sleep 1
    $SUDO docker ps --filter name=$CONTAINER_NAME --format '{{.Status}}'
"
info "${CONTAINER_NAME} running on port ${GUESTKIT_UI_PORT}"

step "Verifying deployment"
BASE_URL="http://${HOST}:${GUESTKIT_UI_PORT}"
DEPLOY_UI_SCHEME="http"
_ssh "curl -fsS http://127.0.0.1:${GUESTKIT_UI_PORT}/ >/dev/null" \
    && info "UI OK (http://127.0.0.1:${GUESTKIT_UI_PORT}/, on-host)"

guestkit_ui_save_deploy_last "$REPO_DIR" "$HOST" "$USER" "ui"

deploy_ui_highlight "📋 Final checklist"
deploy_ui_checklist "container" "$(_ssh_batch "$SUDO docker inspect -f '{{.State.Running}}' $CONTAINER_NAME" | tr -d '\r' | sed 's/true/active/;s/false/exited/')"
deploy_ui_checklist "http"      "$(_ssh_batch "curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:${GUESTKIT_UI_PORT}/" | tr -d '\r')"

guestkit_ui_print_success "$HOST" 0

if $SKIP_SMOKE; then
    info "Skipped smoke-ui-remote.sh (--skip-smoke)"
else
    step "Running scripts/smoke-ui-remote.sh against ${BASE_URL}"
    ( cd "$REPO_DIR" && GUESTKIT_UI_URL="$BASE_URL" ./scripts/smoke-ui-remote.sh )
fi
