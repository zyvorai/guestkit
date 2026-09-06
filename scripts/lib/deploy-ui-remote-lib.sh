# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# shellcheck shell=bash
# Emoji-rich deploy UI (self-contained; no cross-repo imports). Project-agnostic —
# configured entirely through the DEPLOY_UI_* variables set in deploy-common.sh.
# Configure via deploy-common.sh before sourcing.

DEPLOY_UI_PROJECT="${DEPLOY_UI_PROJECT:-hyper2kvm}"
DEPLOY_UI_ICON="${DEPLOY_UI_ICON:-🔄}"
DEPLOY_UI_ICON_UNINSTALL="${DEPLOY_UI_ICON_UNINSTALL:-🗑️}"
DEPLOY_UI_ICON_MAGIC="${DEPLOY_UI_ICON_MAGIC:-✨}"
DEPLOY_UI_PORT="${DEPLOY_UI_PORT:-5070}"
DEPLOY_UI_SCHEME="${DEPLOY_UI_SCHEME:-https}"
DEPLOY_UI_DASH_PATH="${DEPLOY_UI_DASH_PATH:-/}"
DEPLOY_UI_HEALTH_PATH="${DEPLOY_UI_HEALTH_PATH:-/api/v1/health}"

if [ -t 1 ] && command -v tput &>/dev/null && [ "$(tput colors 2>/dev/null || echo 0)" -ge 8 ]; then
    _C_RESET=$'\033[0m' _C_BOLD=$'\033[1m' _C_DIM=$'\033[2m'
    _C_CYAN=$'\033[36m' _C_GREEN=$'\033[32m' _C_YELLOW=$'\033[33m'
    _C_RED=$'\033[31m' _C_MAGENTA=$'\033[35m' _C_BLUE=$'\033[34m'
else
    _C_RESET='' _C_BOLD='' _C_DIM='' _C_CYAN='' _C_GREEN='' _C_YELLOW='' _C_RED='' _C_MAGENTA='' _C_BLUE=''
fi

deploy_ui_step_emoji() {
    local t
    t=$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')
    case "$t" in
        *preflight*|*ssh*|*connect*)     echo "🔌" ;;
        *build*|*compile*)                echo "🔨" ;;
        *sync*|*rsync*|*upload*|*push*)   echo "📤" ;;
        *install*|*deps*|*package*)       echo "📦" ;;
        *python*|*pip*)                   echo "🐍" ;;
        *dashboard*|*web*|*h2kweb*|*npm*) echo "🌐" ;;
        *systemd*|*service*)             echo "⚙️" ;;
        *libvirt*|*kvm*|*qemu*)           echo "🖥️" ;;
        *verify*|*smoke*|*test*|*health*) echo "🩺" ;;
        *clean*|*nuke*)                  echo "🧹" ;;
        *detect*|*runtime*)               echo "🔍" ;;
        *firewall*)                      echo "🔥" ;;
        *uninstall*)                     echo "🗑️" ;;
        *)                               echo "🔧" ;;
    esac
}

deploy_ui_info()  { printf '  %s✅%s %s\n' "$_C_GREEN" "$_C_RESET" "$*"; }
deploy_ui_warn()  { printf '  %s⚠️ %s%s\n' "$_C_YELLOW" "$_C_RESET" "$*"; }
deploy_ui_error() { printf '  %s❌%s %s\n' "$_C_RED" "$_C_RESET" "$*"; exit 1; }
deploy_ui_note()  { printf '  %s💡%s %s\n' "$_C_BLUE" "$_C_RESET" "$*"; }

deploy_ui_kv() {
    local icon="$1" label="$2" value="$3"
    printf '  %s%-14s%s %s\n' "$_C_DIM" "${icon} ${label}" "$_C_RESET" "$value"
}

deploy_ui_banner() {
    local title="$1" subtitle="${2:-}"
    local icon="${3:-$DEPLOY_UI_ICON}"
    printf '\n'
    printf '  %s╔══════════════════════════════════════════════════════════╗%s\n' "$_C_MAGENTA" "$_C_RESET"
    printf '  %s║%s %s %-53s %s║%s\n' "$_C_MAGENTA" "$_C_RESET" "$icon" "$title" "$_C_MAGENTA" "$_C_RESET"
    if [ -n "$subtitle" ]; then
        printf '  %s║%s %s %-53s %s║%s\n' "$_C_MAGENTA" "$_C_RESET" "  " "$subtitle" "$_C_MAGENTA" "$_C_RESET"
    fi
    printf '  %s╚══════════════════════════════════════════════════════════╝%s\n' "$_C_MAGENTA" "$_C_RESET"
    printf '\n'
}

deploy_ui_uninstall_banner() {
    deploy_ui_banner "${DEPLOY_UI_PROJECT} Remote Uninstall" "" "$DEPLOY_UI_ICON_UNINSTALL"
}

deploy_ui_highlight() {
    printf '\n  %s── %s ──%s\n' "$_C_DIM" "$*" "$_C_RESET"
}

_deploy_ui_spinner_pid=""
deploy_ui_spinner_start() {
    local msg="$1"
    [ -t 1 ] || return 0
    (
        local frames=('🌑' '🌒' '🌓' '🌔' '🌕' '🌖' '🌗' '🌘')
        local i=0
        while true; do
            printf '\r  %s %s %s' "${frames[$i]}" "$msg" "$_C_DIM···$_C_RESET"
            i=$(( (i + 1) % 8 ))
            sleep 0.12
        done
    ) &
    _deploy_ui_spinner_pid=$!
}

deploy_ui_spinner_stop() {
    if [ -n "$_deploy_ui_spinner_pid" ]; then
        kill "$_deploy_ui_spinner_pid" 2>/dev/null || true
        wait "$_deploy_ui_spinner_pid" 2>/dev/null || true
        _deploy_ui_spinner_pid=""
    fi
    printf '\r\033[K'
}

DEPLOY_UI_CURRENT_STEP=""
DEPLOY_UI_STEP_TS=0

deploy_ui_step_start() {
    local title="$1"
    local now emoji
    now=$(date +%s)
    emoji=$(deploy_ui_step_emoji "$title")
    if [ -n "${DEPLOY_UI_CURRENT_STEP:-}" ] && [ "${DEPLOY_UI_STEP_TS:-0}" -gt 0 ]; then
        deploy_ui_info "${DEPLOY_UI_CURRENT_STEP} done in $((now - DEPLOY_UI_STEP_TS))s"
    fi
    DEPLOY_UI_CURRENT_STEP="$title"
    DEPLOY_UI_STEP_TS="$now"
    DEPLOY_UI_LAST_ACTION="$title"
    printf '\n  %s%s%s %s\n' "$_C_CYAN" "$emoji" "$_C_RESET" "$title"
}

deploy_ui_step_finish() {
    local now
    now=$(date +%s)
    if [ -n "${DEPLOY_UI_CURRENT_STEP:-}" ] && [ "${DEPLOY_UI_STEP_TS:-0}" -gt 0 ]; then
        deploy_ui_info "${DEPLOY_UI_CURRENT_STEP} done in $((now - DEPLOY_UI_STEP_TS))s"
    fi
    DEPLOY_UI_CURRENT_STEP=""
    DEPLOY_UI_STEP_TS=0
}

deploy_ui_dry_run() {
    local host="$1" user="$2" remote_dir="$3" quick="$4"
    deploy_ui_banner "${DEPLOY_UI_ICON_MAGIC} Dry run" "no changes will be made"
    deploy_ui_kv "🎯" "Target" "${user}@${host}"
    deploy_ui_kv "📁" "Remote dir" "$remote_dir"
    deploy_ui_kv "⚡" "Mode" "$([ "$quick" = true ] && echo 'quick' || echo 'full')"
    echo ""
    deploy_ui_note "Would: build → rsync → install → restart → smoke-test"
    echo ""
}

deploy_ui_checklist() {
    local label="$1" status="$2"
    local icon="✅"
    case "$status" in
        OK|ok|active|200) icon="✅" ;;
        WARN*|warn*)      icon="⚠️" ;;
        *)                icon="❌" ;;
    esac
    printf '    %s %-14s %s\n' "$icon" "$label" "$status"
}

deploy_ui_success() {
    local host="${1:-localhost}" secs="${2:-0}" extra_cmd="${3:-}"
    local base="${DEPLOY_UI_SCHEME}://${host}:${DEPLOY_UI_PORT}"
    printf '\n'
    printf '  %s  %s %s deploy complete! %s %s%s\n' "$DEPLOY_UI_ICON" "$_C_CYAN" "$DEPLOY_UI_PROJECT" "$DEPLOY_UI_ICON_MAGIC" "$_C_RESET"
    printf '  %s   ╭──────────────────────────────────────────────╮%s\n' "$_C_GREEN" "$_C_RESET"
    printf '  %s   │ 🌐 Dashboard  %-30s │%s\n' "$_C_GREEN" "${base}${DEPLOY_UI_DASH_PATH}" "$_C_RESET"
    printf '  %s   │ 💚 Health API  %-30s │%s\n' "$_C_GREEN" "${base}${DEPLOY_UI_HEALTH_PATH}" "$_C_RESET"
    printf '  %s   ╰──────────────────────────────────────────────╯%s\n' "$_C_GREEN" "$_C_RESET"
    [ -n "$secs" ] && [ "$secs" -gt 0 ] 2>/dev/null && printf '  %s⏱️  %ss total%s\n' "$_C_DIM" "$secs" "$_C_RESET"
    if [ -n "$extra_cmd" ]; then
        printf '  %s🔁%s %s\n' "$_C_DIM" "$_C_RESET" "$extra_cmd"
    fi
    printf '\n'
}

deploy_ui_fail() {
    local action="${1:-unknown}" line="${2:-?}" log="${3:-}"
    printf '\n'
    deploy_ui_error "Deploy failed at: ${action} (line ${line})"
    [ -n "$log" ] && [ -f "$log" ] && deploy_ui_note "Log: ${log}"
}

deploy_ui_parse_target() {
    local _host_var="$1" _user_var="$2"
    local target
    eval "target=\${$_host_var}"
    if [[ "$target" == *@* ]]; then
        eval "${_user_var}=\${target%%@*}"
        eval "${_host_var}=\${target#*@}"
    fi
}

deploy_ui_deploy_state_file() {
    echo "$1/.deploy-last"
}

deploy_ui_save_deploy_last() {
    local repo_dir="$1" host="$2" user="$3" mode="$4" version="${5:-}" commit="${6:-}"
    local f
    f=$(deploy_ui_deploy_state_file "$repo_dir")
    cat >"$f" <<EOF
# Auto-generated by ${DEPLOY_UI_PROJECT} deploy (no passwords)
HOST=$host
USER=$user
MODE=$mode
PORT=${DEPLOY_UI_PORT:-}
UPDATED=$(date -u '+%Y-%m-%dT%H:%M:%SZ')
VERSION=${version:-unknown}
COMMIT=${commit:-unknown}
EOF
    chmod 600 "$f" 2>/dev/null || true
}

deploy_ui_load_deploy_last() {
    local repo_dir="$1" f
    f=$(deploy_ui_deploy_state_file "$repo_dir")
    [ -f "$f" ] || return 1
    # shellcheck disable=SC1090
    source "$f"
    return 0
}
