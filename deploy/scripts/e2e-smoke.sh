#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# E2E smoke test for Zyvor API workflow
set -euo pipefail

API="${API:-http://localhost:8080/api/v1}"
IMAGE="${IMAGE:-}"

# head closes pipes early; avoid SIGPIPE under pipefail
json_head() {
  python3 -c "import sys,json; print(json.dumps(json.load(sys.stdin), indent=2))" | head -n "${1:-80}" || true
}

# curl -sf swallows the response body on a non-2xx status, so a real API
# error (bad request, internal error) looks identical to "empty body" —
# the caller's json parse then dies with an unhelpful "Expecting value:
# line 1 column 1" instead of the actual error. curl_or_die separates the
# HTTP status from the body, prints status+body to stderr on failure, and
# only emits the body to stdout on success.
curl_or_die() {
  local resp status body
  resp=$(curl -s -w '\n%{http_code}' "$@")
  status="${resp##*$'\n'}"
  body="${resp%$'\n'*}"
  if [[ "${status}" -lt 200 || "${status}" -ge 300 ]]; then
    echo "  HTTP ${status}: ${body}" >&2
    return 1
  fi
  echo "${body}"
}

poll_job() {
  local job_id="$1"
  local label="${2:-job}"
  for i in $(seq 1 60); do
    local job
    job=$(curl -sf "${API}/jobs/${job_id}" || true)
    local status
    status=$(echo "${job}" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('data',{}).get('live_status',{}).get('status','pending'))" 2>/dev/null || echo pending)
    echo "  ${label} poll ${i}: ${status}" >&2
    if [[ "${status}" == "completed" ]]; then
      echo "${job}"
      return 0
    fi
    if [[ "${status}" == "failed" || "${status}" == "cancelled" || "${status}" == "timeout" ]]; then
      echo "  ${label} FAILED (status=${status}):" >&2
      echo "${job}" | python3 -c "
import sys, json
d = json.load(sys.stdin)
live = d.get('data', {}).get('live_status', {})
err = live.get('error') or d.get('data', {}).get('error')
print(f'    error: {err}', file=sys.stderr)
print(json.dumps(d, indent=2), file=sys.stderr)
" 2>/dev/null || echo "${job}" >&2
      return 1
    fi
    sleep 5
  done
  echo "  (${label} timed out)" >&2
  return 1
}

if [[ -z "${IMAGE}" ]]; then
  echo "Usage: IMAGE=./disk.qcow2 API=http://api.zyvor.local/api/v1 $0"
  exit 1
fi

echo "Importing ${IMAGE}..."
IMPORT=$(curl_or_die -F "file=@${IMAGE}" "${API}/vms/import")
echo "${IMPORT}" | head -c 500
VM_ID=$(echo "${IMPORT}" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['id'])")
DISK_PATH=$(echo "${IMPORT}" | python3 -c "import sys,json; print(json.load(sys.stdin)['data'].get('path',''))" 2>/dev/null || true)
echo ""
echo "VM ID: ${VM_ID}"
[[ -n "${DISK_PATH}" ]] && echo "Disk path: ${DISK_PATH}"

echo "Inspect..."
INSPECT=$(curl_or_die -X POST "${API}/vms/${VM_ID}/inspect")
INS_JOB=$(echo "${INSPECT}" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['job_id'])")
INS_RESULT=$(poll_job "${INS_JOB}" inspect)
python3 -c "import sys,json; d=json.load(sys.stdin); r=d.get('data',{}).get('result',{}); assert r.get('inspect') or r.get('data',{}).get('inspect'), r" <<< "${INS_RESULT}"

echo "Doctor..."
DOCTOR=$(curl_or_die -X POST "${API}/vms/${VM_ID}/doctor?target=kubevirt&explain=true")
JOB_ID=$(echo "${DOCTOR}" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['job_id'])")
DOC_RESULT=$(poll_job "${JOB_ID}" doctor)
echo "${DOC_RESULT}" | json_head 80

echo "Migration plan..."
MIGPLAN=$(curl_or_die -X POST "${API}/vms/${VM_ID}/migration-plan?target=kubevirt")
echo "${MIGPLAN}" | python3 -m json.tool
MIGPLAN_JOB=$(echo "${MIGPLAN}" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['job_id'])")
# Wait for this to finish before provision: migration-plan mounts the disk
# asynchronously via guestkit-worker, and provision below mounts the *same*
# disk synchronously in zyvor-api's own process. Racing them means two
# processes independently attaching a loop/NBD device for the same image —
# find_available_device() (src/disk/nbd.rs) checks device availability and
# connects as two separate, unlocked steps with no cross-process
# coordination, so a race here can make one side's mount fail outright
# ("No operating system found in disk image") rather than just being slow.
poll_job "${MIGPLAN_JOB}" migration-plan >/dev/null

echo "Provision YAML..."
curl_or_die -X POST "${API}/vms/${VM_ID}/provision" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['yaml'][:2000])"

echo "Config endpoint..."
curl_or_die "${API}/config" | json_head 20

if curl -sf "${API}/kubevirt/vms" >/dev/null 2>&1; then
  echo "KubeVirt fleet..."
  curl_or_die "${API}/kubevirt/vms" | python3 -m json.tool | head -40
  echo "KubeVirt namespaces..."
  curl_or_die "${API}/kubevirt/namespaces" | python3 -m json.tool

  echo "Live guest deep intelligence (first running VM)..."
  RUNNING=$(curl -sf "${API}/kubevirt/vms" | python3 -c "
import sys,json
vms=json.load(sys.stdin).get('data',[])
for v in vms:
  phase=str(v.get('phase') or v.get('status') or '').lower()
  if 'run' in phase:
    print(v['namespace'], v['name'])
    break
" || true)
  if [[ -n "${RUNNING}" ]]; then
    GNS=$(echo "${RUNNING}" | awk '{print $1}')
    GNAME=$(echo "${RUNNING}" | awk '{print $2}')
    echo "  VM: ${GNS}/${GNAME}"
    curl -sf "${API}/kubevirt/vms/${GNS}/${GNAME}/guest/info" | json_head 25 || echo "  (guest/info unavailable)"
    curl -s "${API}/kubevirt/vms/${GNS}/${GNAME}/guest/evidence" | json_head 30 || echo "  (guest/evidence unavailable — agent may be missing)"
    curl -s "${API}/kubevirt/vms/${GNS}/${GNAME}/guest/network" | json_head 20 || echo "  (guest/network unavailable)"
    curl -sf "${API}/kubevirt/vms/${GNS}/${GNAME}/guest/health" | json_head 15 || true
    curl -sf "${API}/kubevirt/vms/${GNS}/${GNAME}/guest/journal?limit=5" 2>/dev/null | json_head 15 || echo "  (guest/journal unavailable)"
  else
    echo "  (no running VM — skip live guest intel)"
  fi
fi

if curl -sf "${API}/vmtools/bundle" >/dev/null 2>&1; then
  echo "VM Tools bundle..."
  curl_or_die "${API}/vmtools/bundle" | json_head 20
  echo "VM Tools coverage..."
  curl_or_die "${API}/vmtools/coverage" | python3 -m json.tool
  echo "VM Tools policy..."
  curl_or_die "${API}/vmtools/policy" | python3 -m json.tool
  echo "VM Tools reconcile..."
  RECON=$(curl -sf -X POST "${API}/vmtools/policy/reconcile" || echo '{"success":false}')
  echo "${RECON}" | python3 -m json.tool || true
  python3 -c "import sys,json; d=json.load(sys.stdin); r=d.get('data') or {}; assert 'pending' in r and 'upgraded' in r" <<< "${RECON}" || {
    echo "  (reconcile response missing pending/upgraded — non-fatal)"
  }
fi

if curl -sf "${API}/kubevirt/vms" >/dev/null 2>&1; then
  cluster_inspect_done=0

  if [[ "${E2E_KUBEVIRT:-}" == "1" ]] && command -v kubectl >/dev/null 2>&1 && [[ -n "${DISK_PATH}" && -f "${DISK_PATH}" ]]; then
    echo "KubeVirt cluster inspect E2E (provision halted VM from import)..."
    VM_NAME="smoke-kv-$(date +%s | tail -c 7)"
    NS="${E2E_KUBEVIRT_NS:-zyvor-e2e}"
    PV_NAME="${VM_NAME}-pv"
    PVC_NAME="${VM_NAME}-disk"

    kubectl create namespace "${NS}" --dry-run=client -o yaml | kubectl apply -f -

    kubectl apply -f - <<EOF
apiVersion: v1
kind: PersistentVolume
metadata:
  name: ${PV_NAME}
spec:
  capacity:
    storage: 32Gi
  accessModes:
    - ReadWriteOnce
  persistentVolumeReclaimPolicy: Retain
  storageClassName: ""
  hostPath:
    path: ${DISK_PATH}
    type: File
---
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: ${PVC_NAME}
  namespace: ${NS}
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 32Gi
  storageClassName: ""
  volumeName: ${PV_NAME}
---
apiVersion: kubevirt.io/v1
kind: VirtualMachine
metadata:
  name: ${VM_NAME}
  namespace: ${NS}
spec:
  runStrategy: Halted
  template:
    metadata:
      labels:
        kubevirt.io/vm: ${VM_NAME}
    spec:
      domain:
        devices:
          disks:
            - name: disk0
              disk:
                bus: virtio
        resources:
          requests:
            memory: 512Mi
      volumes:
        - name: disk0
          persistentVolumeClaim:
            claimName: ${PVC_NAME}
EOF

    sleep 3
    CINS=$(curl_or_die -X POST "${API}/kubevirt/vms/${NS}/${VM_NAME}/inspect")
    CINS_JOB=$(echo "${CINS}" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['job_id'])")
    CINS_RESULT=$(poll_job "${CINS_JOB}" cluster-inspect)
    python3 -c "import sys,json; d=json.load(sys.stdin); r=d.get('data',{}).get('result',{}); assert r.get('inspect') or r.get('data',{}).get('inspect'), r" <<< "${CINS_RESULT}"
    cluster_inspect_done=1
    echo "  cluster inspect OK: ${NS}/${VM_NAME}"
  fi

  if [[ "${cluster_inspect_done}" -eq 0 ]]; then
    echo "KubeVirt inspect job (first stopped Linux VM)..."
    STOPPED=$(curl -sf "${API}/kubevirt/vms" | python3 -c "
import sys,json
vms=json.load(sys.stdin).get('data',[])
for v in vms:
  phase=str(v.get('phase') or v.get('status') or '').lower()
  if not v.get('is_windows') and 'run' not in phase:
    print(v['namespace'], v['name'])
    break
" || true)
    if [[ -n "${STOPPED}" ]]; then
      NS=$(echo "${STOPPED}" | awk '{print $1}')
      NAME=$(echo "${STOPPED}" | awk '{print $2}')
      CINS=$(curl_or_die -X POST "${API}/kubevirt/vms/${NS}/${NAME}/inspect")
      CINS_JOB=$(echo "${CINS}" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['job_id'])")
      CINS_RESULT=$(poll_job "${CINS_JOB}" cluster-inspect)
      python3 -c "import sys,json; d=json.load(sys.stdin); r=d.get('data',{}).get('result',{}); assert r.get('inspect') or r.get('data',{}).get('inspect'), r" <<< "${CINS_RESULT}"
    else
      echo "  (no stopped Linux VM — set E2E_KUBEVIRT=1 to provision from import)"
    fi
  fi
fi

if curl -sf "${API}/storage/roots" >/dev/null 2>&1; then
  echo "Storage roots..."
  curl_or_die "${API}/storage/roots" | json_head 20
fi

echo "Smoke test complete."
