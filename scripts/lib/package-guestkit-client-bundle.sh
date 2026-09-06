# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# shellcheck shell=bash
# Assemble GuestKit customer tarball layout (shared by remote pack and GitHub release).
#
# Usage:
#   package_guestkit_client_bundle STAGE BUILD_DIR VERSION
#
# Expects guestkit binary at ${BUILD_DIR}/target/release/guestkit unless
# GUESTKIT_BINARY overrides the path.

package_guestkit_client_bundle() {
    local stage="$1" build_dir="$2" version="$3"
    local binary="${GUESTKIT_BINARY:-${build_dir}/target/release/guestkit}"
    local lib="${build_dir}/scripts/lib"

    rm -rf "${stage}"
    mkdir -p "${stage}"

    if [[ ! -x "${binary}" ]]; then
        echo "package_guestkit_client_bundle: missing executable ${binary}" >&2
        return 1
    fi

    cp "${binary}" "${stage}/guestkit"
    chmod +x "${stage}/guestkit"
    ln -sf guestkit "${stage}/guestctl"

    sed 's#")/\.\.#")#' "${build_dir}/scripts/selftest.sh" > "${stage}/test-selftest.sh"
    chmod +x "${stage}/test-selftest.sh"
    chmod +x "${lib}/copy-zyvor-legal-to-bundle.sh"
    "${lib}/copy-zyvor-legal-to-bundle.sh" "${stage}" "${build_dir}" --with-accept

    cat > "${stage}/guestkit.env.example" <<'ENV_EOF'
# Optional — copy to guestkit.env
# GUESTKIT_LOG=info
# GUESTKIT_CACHE_DIR=$HOME/.cache/guestkit
ENV_EOF

    for f in package-install.sh package-client-install.sh package-client-test.sh \
        package-host-test.sh package-uninstall.sh package-uninstall-lib.sh; do
        if [[ ! -f "${lib}/${f}" ]]; then
            echo "package_guestkit_client_bundle: missing ${lib}/${f}" >&2
            return 1
        fi
    done

    cp "${lib}/package-install.sh" "${stage}/install.sh"
    cp "${lib}/package-client-install.sh" "${stage}/install-client-deps.sh"
    cp "${lib}/package-client-test.sh" "${stage}/test-package.sh"
    cp "${lib}/package-host-test.sh" "${stage}/test-host.sh"
    mkdir -p "${stage}/.package-lib"
    cp "${lib}/package-ui.sh" "${stage}/.package-lib/"
    cp "${lib}/install-everything.sh" "${stage}/"
    cp "${lib}/package-uninstall-lib.sh" "${stage}/.package-lib/"
    cp "${lib}/package-uninstall.sh" "${stage}/uninstall.sh"
    cp "${lib}/HOST_SETUP.txt" "${lib}/PREREQUISITES.txt" "${stage}/"
    chmod +x "${stage}/install.sh" "${stage}/install-client-deps.sh" \
        "${stage}/test-package.sh" "${stage}/test-host.sh" \
        "${stage}/install-everything.sh" "${stage}/uninstall.sh"
    chmod +x "${lib}/write-customer-help.sh"
    "${lib}/write-customer-help.sh" "${stage}" "GuestKit" host
    cp "${lib}/START_HERE.txt" "${stage}/"

    cat > "${stage}/QUICKSTART.txt" <<'QEOF'
GuestKit — install guide
========================

HOST FIRST (Linux — offline disk inspection, not Kubernetes)
  1. tar xzf guestkit-*-linux-amd64.tar.gz && cd guestkit-*-linux-amd64
  2. Read LICENSE (Apache 2.0) and ZYVOR-COMPANY-TERMS.md — ./install.sh prompts ACCEPT
  3. ./install.sh
  4. ./test-host.sh
  5. ./test-selftest.sh --quick
  6. ./guestkit inspect /path/to/vm.qcow2
     (or ./guestctl vm.qcow2 for shorthand inspect)

Checklist: PREREQUISITES.txt  |  Details: HOST_SETUP.txt
Remove: ./uninstall.sh --yes [--remove-dir]

Packaged by Zyvor — zyvor.dev · HyperSDK · © 2026
QEOF

    if [[ -f "${build_dir}/scripts/zyvor-branding/ZYVOR_INSTALL.txt" ]]; then
        cp "${build_dir}/scripts/zyvor-branding/ZYVOR_INSTALL.txt" "${stage}/ZYVOR_INSTALL.txt"
    fi

    cat > "${stage}/README.txt" <<README_EOF
GuestKit ${version} — Linux amd64 client bundle
===============================================

START: cat START_HERE.txt  |  full help: cat HELP.txt

NOT KUBERNETES — inspects offline VM disk images on this Linux host.

FILES
  LICENSE               Apache-2.0 (source code — ZyvorAI Labs Private Limited)
  NOTICE                Copyright and attribution (Apache 2.0)
  ZYVOR-COMPANY-TERMS.md  Zyvor distribution terms (read before install)
  LEGAL-INDEX.txt
  guestkit              Main CLI binary
  guestctl              Symlink to guestkit (same CLI)
  install.sh            Client install (deps + verify)
  install-client-deps.sh  qemu-img, nbd (GuestKit pure Rust — not legacy appliance tooling)
  test-host.sh          Host prerequisite checks
  test-selftest.sh      Full GuestKit selftest
  test-package.sh       Quick smoke test
  uninstall.sh          Remove client install
  HOST_SETUP.txt        Step-by-step + troubleshooting
  PREREQUISITES.txt     Checklist

REQUIREMENTS — see PREREQUISITES.txt
  qemu-img, nbd module, disk image file access (legacy appliance tooling not required)

ORDER: ./install.sh → ./test-host.sh → ./guestkit inspect <image>

UNINSTALL: ./uninstall.sh --yes [--remove-dir]
README_EOF

    local req
    for req in HELP.txt START_HERE.txt install.sh uninstall.sh README.txt QUICKSTART.txt \
        HOST_SETUP.txt PREREQUISITES.txt install-client-deps.sh test-host.sh test-package.sh \
        test-selftest.sh guestkit guestctl guestkit.env.example \
        LEGAL-INDEX.txt ZYVOR-COMPANY-TERMS.md LICENSE; do
        if [[ ! -e "${stage}/${req}" ]]; then
            echo "package_guestkit_client_bundle: bundle missing ${req}" >&2
            return 1
        fi
    done

    chmod +x "${lib}/finalize-customer-bundle.sh"
    "${lib}/finalize-customer-bundle.sh" "${stage}" "${build_dir}" "GuestKit" "${version}" || return 1

    echo "Customer bundle OK"
}

package_guestkit_client_tarball() {
    local out_dir="$1" artifact="$2" stage="$3"
    mkdir -p "${out_dir}"
    (
        cd "${out_dir}"
        tar czf "${artifact}.tar.gz" "$(basename "${stage}")"
        sha256sum "${artifact}.tar.gz" | tee "${artifact}.tar.gz.sha256"
    )
}
