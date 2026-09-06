#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Run on remote host: pleasant end-to-end user install tests for all *-dist tarballs.
#
# Usage:
#   scp scripts/test-packages-remote-only.sh user@host:~/
#   ssh user@host 'bash ~/test-packages-remote-only.sh'
#
# Env:
#   ZYVOR_E2E_INSTALL=install-everything|install   (default: install-everything when present)
#   ZYVOR_E2E_TIMEOUT_SECS=900                    default per-product install timeout
#   ZYVOR_E2E_SKIP=vmrogue,machina                comma-separated product names to skip
set -uo pipefail

TEST_ROOT="${HOME}/package-tests"
mkdir -p "${TEST_ROOT}"
RUN_ID="$(date +%Y%m%d-%H%M%S)"
RESULTS="${TEST_ROOT}/results-${RUN_ID}.log"
PASS=0 FAIL=0 WARN=0
SESSION_START=${SECONDS}

# Colors when stdout is a TTY
if [[ -t 1 ]]; then
  C_GREEN=$'\033[0;32m' C_RED=$'\033[0;31m' C_YELLOW=$'\033[0;33m'
  C_CYAN=$'\033[0;36m' C_BOLD=$'\033[1m' C_DIM=$'\033[2m' C_RESET=$'\033[0m'
else
  C_GREEN= C_RED= C_YELLOW= C_CYAN= C_BOLD= C_DIM= C_RESET=
fi

log() { echo "$@" | tee -a "${RESULTS}"; }
log_ok() { log "${C_GREEN}✓${C_RESET} $*"; }
log_fail() { log "${C_RED}✗${C_RESET} $*"; }
log_warn() { log "${C_YELLOW}!${C_RESET} $*"; }
log_phase() { log ""; log "${C_BOLD}${C_CYAN}━━ $* ━━${C_RESET}"; }

pick_latest() { ls -t "$1"/*.tar.gz 2>/dev/null | head -1; }

tar_has_file() {
  local tarball="$1" leaf="$2"
  tar tzf "${tarball}" 2>/dev/null | awk -F/ -v leaf="${leaf}" '$NF==leaf{found=1} END{exit !found}'
}

should_skip() {
  local name="$1"
  local skip="${ZYVOR_E2E_SKIP:-}"
  [[ -z "${skip}" ]] && return 1
  local want="${name,,}" item
  IFS=',' read -ra _skip_list <<< "${skip}"
  for item in "${_skip_list[@]}"; do
    item="${item//[[:space:]]/}"
    [[ -z "${item}" ]] && continue
    [[ "${item,,}" == "${want}" ]] && return 0
  done
  return 1
}

install_timeout_for() {
  local name="$1"
  case "${name,,}" in
    machina|vmrogue|v9s|ragnarok|aether|ironwolf|packetwolf) echo 900 ;;
    *) echo "${ZYVOR_E2E_TIMEOUT_SECS:-600}" ;;
  esac
}

choose_install_cmd() {
  local mode="${ZYVOR_E2E_INSTALL:-}"
  if [[ "${mode}" == "install" ]]; then
    echo "./install.sh"
    return
  fi
  if [[ "${mode}" == "install-everything" ]] || [[ -x ./install-everything.sh ]]; then
    echo "./install-everything.sh"
    return
  fi
  echo "./install.sh"
}

verify_bundle_layout() {
  local tarball="$1"
  local ok=0
  for f in uninstall.sh install.sh START_HERE.txt HELP.txt install-everything.sh; do
    if tar_has_file "${tarball}" "${f}"; then
      log_ok "  tarball contains ${f}"
    else
      log_warn "  tarball missing ${f}"
      ok=1
    fi
  done
  tar_has_file "${tarball}" "package-ui.sh" || log_warn "  package-ui.sh not under .package-lib (old bundle?)"
  return "${ok}"
}

verify_user_pdfs() {
  local ok=0
  if [[ -d ./docs/pdf ]]; then
    [[ -f ./docs/welcome.html ]] && log_ok "  docs/welcome.html" || { log_fail "  missing docs/welcome.html"; ok=1; }
    [[ -f ./docs/pdf/WELCOME.pdf ]] && log_ok "  docs/pdf/WELCOME.pdf" || { log_fail "  missing docs/pdf/WELCOME.pdf"; ok=1; }
    [[ -f ./OPEN_FIRST.txt ]] && log_ok "  OPEN_FIRST.txt" || log_warn "  missing OPEN_FIRST.txt"
    [[ -f ./docs/zyvor-logo.png ]] && log_ok "  docs/zyvor-logo.png" || { log_warn "  missing docs/zyvor-logo.png"; ok=1; }
    [[ -f ./docs/PDF_INDEX.txt ]] && log_ok "  docs/PDF_INDEX.txt" || log_warn "  missing docs/PDF_INDEX.txt"
  else
    log_warn "  docs/pdf/ not bundled (old tarball?)"
    ok=1
  fi
  return "${ok}"
}

verify_script_paths() {
  local ok=0 f
  local -a root_scripts=(
    install-cluster.sh apply-cluster-network.sh test-cluster.sh test-host.sh
    install.sh install-everything.sh install-client-deps.sh test-package.sh uninstall.sh
  )
  for f in "${root_scripts[@]}"; do
    [[ -f "./${f}" ]] || continue
    if grep -q 'dirname "$0")/\.\.' "./${f}"; then
      log_fail "  ${f} uses parent-dir ROOT (dirname \$0)/.. — repack required"
      ok=1
    fi
  done
  if [[ -x ./install-cluster.sh ]]; then
    [[ -f ./cluster/install-cluster-prereqs.sh ]] || {
      log_fail "  install-cluster.sh missing cluster/install-cluster-prereqs.sh beside it"
      ok=1
    }
    if ! ./install-cluster.sh --help >/dev/null 2>&1; then
      log_fail "  install-cluster.sh --help failed (run from extract dir only)"
      ok=1
    else
      log_ok "  install-cluster.sh path resolution"
    fi
  fi
  return "${ok}"
}

verify_extracted_ux() {
  local ok=0
  [[ -f START_HERE.txt ]] && grep -q 'install-everything' START_HERE.txt && log_ok "  START_HERE.txt" || { log_warn "  START_HERE.txt"; ok=1; }
  [[ -f HELP.txt ]] && log_ok "  HELP.txt ($(wc -l < HELP.txt | tr -d ' ') lines)" || { log_warn "  HELP.txt"; ok=1; }
  if [[ -x ./install-everything.sh ]]; then
    if ./install-everything.sh --help 2>/dev/null | grep -q 'install-everything\|HELP\|Zyvor\|START'; then
      log_ok "  install-everything.sh --help"
    else
      log_warn "  install-everything.sh --help"
      ok=1
    fi
  fi
  if [[ -x ./install.sh ]]; then
    ./install.sh --help >/dev/null 2>&1 && log_ok "  install.sh --help" || { log_warn "  install.sh --help"; ok=1; }
  fi
  return "${ok}"
}

test_tarball() {
  local name="$1" tarball="$2"
  if should_skip "${name}"; then
    log_phase "${name} (skipped)"
    return 0
  fi

  log_phase "${name}"
  local t0=${SECONDS}

  [[ -f "${tarball}" ]] || { log_fail "missing tarball: ${tarball}"; ((FAIL++)); return 1; }
  log "  tarball: ${tarball}"

  tar_has_file "${tarball}" "uninstall.sh" || { log_fail "no uninstall.sh in archive"; ((FAIL++)); return 1; }

  verify_bundle_layout "${tarball}" || ((WARN++))

  local work="${TEST_ROOT}/${name}-${RUN_ID}-$$"
  rm -rf "${work}" && mkdir -p "${work}"
  tar xzf "${tarball}" -C "${work}" || { log_fail "extract failed"; ((FAIL++)); return 1; }

  local dir
  dir=$(find "${work}" -maxdepth 1 -mindepth 1 -type d | head -1)
  cd "${dir}" || { log_fail "no extract directory"; ((FAIL++)); return 1; }
  log "  extracted: ${dir}"

  verify_extracted_ux || ((WARN++))
  verify_script_paths || { ((FAIL++)); rm -rf "${work}"; return 1; }
  verify_user_pdfs || ((WARN++))

  local install_cmd timeout_secs
  install_cmd="$(choose_install_cmd)"
  timeout_secs="$(install_timeout_for "${name}")"
  log "  install: ${install_cmd} (timeout ${timeout_secs}s, ZYVOR_NONINTERACTIVE=1)"

  if ! timeout "${timeout_secs}" env ZYVOR_NONINTERACTIVE=1 ZYVOR_AUTO_INSTALL=0 KUBECONFIG= bash -c "${install_cmd}" </dev/null >>"${RESULTS}" 2>&1; then
    log_fail "${install_cmd} failed or timed out (${timeout_secs}s)"
    tail -30 "${RESULTS}" | tee -a "${RESULTS}" >/dev/null
    rm -rf "${work}"
    ((FAIL++))
    return 1
  fi
  log_ok "${install_cmd}"

  if [[ -x ./test-package.sh ]]; then
    ./test-package.sh >>"${RESULTS}" 2>&1 && log_ok "test-package.sh" || { log_warn "test-package.sh"; ((WARN++)); }
  fi

  if [[ -x ./test-cluster.sh ]] && command -v kubectl >/dev/null 2>&1 \
    && [[ -n "${KUBECONFIG:-}" ]] && [[ -f "${KUBECONFIG}" ]]; then
    ./test-cluster.sh >>"${RESULTS}" 2>&1 && log_ok "test-cluster.sh" || log_warn "test-cluster.sh (optional)"
  fi

  if [[ -x ./test-host.sh ]]; then
    ./test-host.sh >>"${RESULTS}" 2>&1 && log_ok "test-host.sh" || log_warn "test-host.sh (optional)"
  fi

  ./uninstall.sh --yes --remove-dir >>"${RESULTS}" 2>&1 || {
    log_fail "uninstall.sh"
    rm -rf "${work}"
    ((FAIL++))
    return 1
  }
  sleep 2
  if [[ -d "${dir}" ]]; then
    log_fail "install directory still exists after uninstall"
    rm -rf "${work}"
    ((FAIL++))
    return 1
  fi

  local elapsed=$((SECONDS - t0))
  log_ok "PASS ${name} (${elapsed}s)"
  ((PASS++))
  return 0
}

declare -a JOBS=(
  "VMRogue|$(pick_latest "${HOME}/vmrogue-dist")"
  "machina|$(pick_latest "${HOME}/machina-dist")"
  "v9s|$(pick_latest "${HOME}/v9s-dist")"
  "guestkit|$(pick_latest "${HOME}/guestkit-dist")"
  "hypersdk|$(pick_latest "${HOME}/hypersdk-dist")"
  "hyper2kvm|$(pick_latest "${HOME}/hyper2kvm-dist")"
  "packetwolf|$(pick_latest "${HOME}/packetwolf-dist")"
  "ragnarok|$(pick_latest "${HOME}/ragnarok-dist")"
  "Aether|$(pick_latest "${HOME}/aether-dist")"
  "IronWolf|$(pick_latest "${HOME}/ironwolf-dist")"
  "forge|$(pick_latest "${HOME}/forge-dist")"
)

log "${C_BOLD}Zyvor user bundle E2E (remote)${C_RESET}"
log "  install mode: ${ZYVOR_E2E_INSTALL:-install-everything (when bundled)}"
log "  log file: ${RESULTS}"
log ""

for job in "${JOBS[@]}"; do
  test_tarball "${job%%|*}" "${job#*|}" || true
done

total_elapsed=$((SECONDS - SESSION_START))
log ""
log "${C_BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${C_RESET}"
log "${C_BOLD}Summary${C_RESET}  ${C_GREEN}${PASS} passed${C_RESET}  ${C_RED}${FAIL} failed${C_RESET}  ${C_YELLOW}${WARN} warnings${C_RESET}  (${total_elapsed}s)"
log "Full log: ${RESULTS}"
exit $((FAIL > 0))
