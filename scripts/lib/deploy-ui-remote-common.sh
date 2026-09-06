# SPDX-License-Identifier: Apache-2.0
# shellcheck shell=bash
# GuestKit UI remote-deploy helpers (self-contained under scripts/lib/).

_DEPLOY_LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

DEPLOY_UI_PROJECT="guestkit-ui"
DEPLOY_UI_ICON="🖥️"
DEPLOY_UI_ICON_UNINSTALL="🗑️"
DEPLOY_UI_ICON_MAGIC="✨"
DEPLOY_UI_PORT="${GUESTKIT_UI_PORT:-0}"
DEPLOY_UI_SCHEME="http"
DEPLOY_UI_DASH_PATH="/"
DEPLOY_UI_HEALTH_PATH="/"

# shellcheck source=deploy-ui-remote-lib.sh
source "$_DEPLOY_LIB_DIR/deploy-ui-remote-lib.sh"

guestkit_ui_build_metadata() {
    local repo_dir="$1"
    GUESTKIT_GIT_VERSION=$(git -C "$repo_dir" describe --tags --always --dirty 2>/dev/null || echo 'dev')
    GUESTKIT_GIT_COMMIT=$(git -C "$repo_dir" rev-parse --short HEAD 2>/dev/null || echo 'unknown')
    export GUESTKIT_GIT_VERSION GUESTKIT_GIT_COMMIT
}

guestkit_ui_parse_target() { deploy_ui_parse_target "$@"; }
guestkit_ui_save_deploy_last() {
    deploy_ui_save_deploy_last "$1" "$2" "$3" "$4" "${GUESTKIT_GIT_VERSION:-}" "${GUESTKIT_GIT_COMMIT:-}"
}
guestkit_ui_load_deploy_last() { deploy_ui_load_deploy_last "$1"; }
guestkit_ui_print_success() {
    deploy_ui_success "$1" "$2" "./scripts/deploy-ui-remote.sh $1 --uninstall"
}

guestkit_ui_info()  { deploy_ui_info "$@"; }
guestkit_ui_warn()  { deploy_ui_warn "$@"; }
guestkit_ui_error() { deploy_ui_error "$@"; }
