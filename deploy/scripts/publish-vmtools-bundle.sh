#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Build Zeus VM Tools artifacts and publish to MinIO (or any S3-compatible store).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
VERSION="${VERSION:-0.1.0}"
BUCKET="${VMTOOLS_BUCKET:-vmtools}"
PREFIX="${VMTOOLS_PREFIX:-}"
MINIO_ENDPOINT="${MINIO_ENDPOINT:-http://minio:9000}"
MINIO_ACCESS_KEY="${MINIO_ACCESS_KEY:-zyvor}"
MINIO_SECRET_KEY="${MINIO_SECRET_KEY:-zyvor-secret}"

cd "${ROOT}"
bash packaging/vmtools/build-artifacts.sh

if command -v mc >/dev/null 2>&1; then
  mc alias set zyvor "${MINIO_ENDPOINT}" "${MINIO_ACCESS_KEY}" "${MINIO_SECRET_KEY}"
  mc mb -p "zyvor/${BUCKET}" 2>/dev/null || true
  mc cp --recursive dist/vmtools/ "zyvor/${BUCKET}/${PREFIX}"
  echo "Published to ${MINIO_ENDPOINT}/${BUCKET}/${PREFIX}"
elif [[ "${MINIO_VIA_KUBECTL:-}" == "1" ]] && command -v kubectl >/dev/null 2>&1; then
  NS="${MINIO_NAMESPACE:-zyvor}"
  # `mc`'s built-in "local" alias defaults to the stock minioadmin:minioadmin
  # credentials, which don't match this deployment's MINIO_ROOT_USER/PASSWORD
  # (templates/secrets.yaml, from values.yaml's minio.accessKey/secretKey) —
  # every mc call below fails with "Insufficient permissions" without this.
  kubectl -n "${NS}" exec deploy/minio -- \
    mc alias set local "http://localhost:9000" "${MINIO_ACCESS_KEY}" "${MINIO_SECRET_KEY}"
  kubectl -n "${NS}" exec deploy/minio -- mc mb "local/${BUCKET}" 2>/dev/null || true
  while IFS= read -r -d '' file; do
    rel="${file#dist/vmtools/}"
    key="${PREFIX}${rel}"
    echo "  upload ${key}..."
    cat "${file}" | kubectl -n "${NS}" exec -i deploy/minio -- sh -c "cat > /tmp/upload && mc cp /tmp/upload local/${BUCKET}/${key} && rm -f /tmp/upload"
  done < <(find dist/vmtools -type f -print0)
  kubectl -n "${NS}" exec deploy/minio -- mc anonymous set download "local/${BUCKET}" 2>/dev/null || true
  echo "Published via kubectl→minio to ${BUCKET}/${PREFIX}"
elif command -v aws >/dev/null 2>&1; then
  export AWS_ACCESS_KEY_ID="${MINIO_ACCESS_KEY}"
  export AWS_SECRET_ACCESS_KEY="${MINIO_SECRET_KEY}"
  aws --endpoint-url "${MINIO_ENDPOINT}" s3 mb "s3://${BUCKET}" 2>/dev/null || true
  aws --endpoint-url "${MINIO_ENDPOINT}" s3 sync dist/vmtools/ "s3://${BUCKET}/${PREFIX}"
  echo "Published to s3://${BUCKET}/${PREFIX}"
else
  echo "Install minio client (mc) or aws CLI to upload artifacts."
  echo "Artifacts ready under dist/vmtools/"
  exit 1
fi
