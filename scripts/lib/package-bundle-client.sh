# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# shellcheck shell=bash
# Source from package-binary-remote.sh REMOTE_PACK blocks.
# Usage: package_bundle_client_scripts STAGE BUILD_DIR [PRODUCT_NAME] [KIND]
#   KIND: k8s | host | platform | minimal (default minimal)
package_bundle_client_scripts() {
    local stage="$1" build_dir="$2"
    local product="${3:-Client}"
    local kind="${4:-minimal}"
    local lib="${build_dir}/scripts/lib"
    for src in package-install.sh package-client-install.sh package-client-test.sh; do
        if [ ! -f "${lib}/${src}" ]; then
            echo "ERROR: missing ${lib}/${src} — cannot build customer tarball" >&2
            return 1
        fi
    done
    cp "${lib}/package-install.sh" "${stage}/install.sh"
    cp "${lib}/package-client-install.sh" "${stage}/install-client-deps.sh"
    cp "${lib}/package-client-test.sh" "${stage}/test-package.sh"
    chmod +x "${stage}/install.sh" "${stage}/install-client-deps.sh" "${stage}/test-package.sh"

    mkdir -p "${stage}/.package-lib"
    cp "${lib}/package-ui.sh" "${stage}/.package-lib/"
    cp "${lib}/install-everything.sh" "${stage}/"
    cp "${lib}/package-uninstall-lib.sh" "${stage}/.package-lib/"
    cp "${lib}/package-uninstall.sh" "${stage}/uninstall.sh"
    chmod +x "${stage}/install-everything.sh" "${stage}/uninstall.sh"

    package_bundle_customer_docs "${stage}" "${build_dir}" "${product}" "${kind}"
}

# HELP.txt, START_HERE.txt, ZYVOR_INSTALL.txt
package_bundle_customer_docs() {
    local stage="$1" build_dir="$2" product="$3" kind="${4:-minimal}"
    local lib="${build_dir}/scripts/lib"
    if [[ -x "${lib}/write-customer-help.sh" ]]; then
        "${lib}/write-customer-help.sh" "${stage}" "${product}" "${kind}"
    else
        echo "WARN: missing ${lib}/write-customer-help.sh" >&2
    fi
    [[ -f "${lib}/START_HERE.txt" ]] && cp "${lib}/START_HERE.txt" "${stage}/"
    [[ -f "${build_dir}/scripts/zyvor-branding/ZYVOR_INSTALL.txt" ]] && \
        cp "${build_dir}/scripts/zyvor-branding/ZYVOR_INSTALL.txt" "${stage}/" 2>/dev/null || true

    package_bundle_customer_pdfs "${stage}" "${build_dir}" "${product}"
}

# Branded PDF copies of customer .txt docs → docs/pdf/ + docs/zyvor-logo.png
package_bundle_customer_pdfs() {
    local stage="$1" build_dir="$2" product="$3"
    local version="${4:-${V9S_PACKAGE_VERSION:-latest}}"
    local lib="${build_dir}/scripts/lib"
    if [[ ! -x "${lib}/generate-customer-pdfs.sh" ]]; then
        echo "WARN: missing ${lib}/generate-customer-pdfs.sh — skipping PDF docs" >&2
        return 0
    fi
    chmod +x "${lib}/generate-customer-pdfs.sh"
    "${lib}/generate-customer-pdfs.sh" "${stage}" "${build_dir}" "${product}" "${version}"
}
