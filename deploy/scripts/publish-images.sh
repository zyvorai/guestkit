#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Build and publish Zyvor container images to GHCR.
#
# Usage:
#   ./deploy/scripts/publish-images.sh
#   REGISTRY=ghcr.io/hypersdk TAG=v0.4.0 ./deploy/scripts/publish-images.sh
#   PUSH=0 ./deploy/scripts/publish-images.sh   # build only
#
# Deploy published images (on cluster host):
#   REGISTRY=ghcr.io/hypersdk TAG=latest helm upgrade --install zyvor deploy/helm/zyvor \
#     --set guestkitWorker.image=ghcr.io/hypersdk/guestkit-worker:latest \
#     --set zyvorApi.image=ghcr.io/hypersdk/zyvor-api:latest \
#     --set zyvorUi.image=ghcr.io/hypersdk/zyvor-ui:latest
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
REGISTRY="${REGISTRY:-ghcr.io/hypersdk}"
TAG="${TAG:-latest}"
BUILDER="${BUILDER:-podman}"
PUSH="${PUSH:-1}"
GITHUB_USER="${GITHUB_USER:-$(gh api user -q .login 2>/dev/null || echo "")}"

cd "${ROOT}"

if ! command -v "${BUILDER}" >/dev/null; then
  echo "ERROR: ${BUILDER} not found"
  exit 1
fi

if [[ "${PUSH}" == "1" ]]; then
  if [[ -z "${GITHUB_USER}" ]]; then
    echo "ERROR: set GITHUB_USER or install gh and login"
    exit 1
  fi
  TOKEN="${GITHUB_TOKEN:-$(gh auth token 2>/dev/null || true)}"
  if [[ -z "${TOKEN}" ]]; then
    echo "ERROR: set GITHUB_TOKEN or install gh and login"
    exit 1
  fi
  echo "Logging in to ${REGISTRY} as ${GITHUB_USER}..."
  echo "${TOKEN}" | "${BUILDER}" login "${REGISTRY}" -u "${GITHUB_USER}" --password-stdin
fi

build_push() {
  local name="$1"
  local dockerfile="$2"
  local context="$3"
  local image="${REGISTRY}/${name}:${TAG}"
  echo "=== Building ${image} ==="
  local build_args=(-t "${image}" -f "${dockerfile}" "${context}")
  if [[ "${BUILDER}" == "podman" ]]; then
    build_args=(--format docker "${build_args[@]}")
  fi
  "${BUILDER}" build "${build_args[@]}"
  if [[ "${TAG}" != "latest" ]]; then
    "${BUILDER}" tag "${image}" "${REGISTRY}/${name}:latest"
  fi
  if [[ "${PUSH}" == "1" ]]; then
    echo "=== Pushing ${image} ==="
    "${BUILDER}" push "${image}"
    if [[ "${TAG}" != "latest" ]]; then
      "${BUILDER}" push "${REGISTRY}/${name}:latest"
    fi
  fi
}

build_push guestkit-worker "${ROOT}/crates/guestkit-worker/Dockerfile" "${ROOT}"
build_push zyvor-api "${ROOT}/crates/zyvor-api/Dockerfile" "${ROOT}"
build_push zyvor-ui "${ROOT}/deploy/ui/Dockerfile" "${ROOT}/deploy/ui"

echo ""
echo "=== Published ==="
echo "  ${REGISTRY}/guestkit-worker:${TAG}"
echo "  ${REGISTRY}/zyvor-api:${TAG}"
echo "  ${REGISTRY}/zyvor-ui:${TAG}"
if [[ "${TAG}" != "latest" ]]; then
  echo "  (also tagged :latest)"
fi
