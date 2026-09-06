#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# End-to-end: Photon OS OVA → GuestKit inspect/doctor → KubeVirt VM → cluster inspect.
set -euo pipefail

API="${API:-http://127.0.0.1:30080/api/v1}"
NS="${NS:-default}"
STORAGE="${STORAGE:-/var/lib/zyvor/images}"
WORKDIR="${STORAGE}/e2e-photon"
OVA_URL="${OVA_URL:-https://packages.vmware.com/photon/5.0/GA/ova/photon-hw15-5.0-dde71ec57.x86_64.ova}"
QCOW="${WORKDIR}/photon.qcow2"
VM_KV_NAME="${VM_KV_NAME:-photon-e2e}"
TAG="$(date +%Y%m%d%H%M%S)"

log() { echo "[e2e-photon] $*" >&2; }

poll_job() {
  local job_id="$1"
  local label="${2:-job}"
  for i in $(seq 1 120); do
    local job
    job=$(curl -sf "${API}/jobs/${job_id}" || echo '{}')
    local status
    status=$(echo "${job}" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('data',{}).get('live_status',{}).get('status', d.get('data',{}).get('status','pending')))" 2>/dev/null || echo pending)
    log "${label} poll ${i}: ${status}"
    if [[ "${status}" == "completed" ]]; then
      echo "${job}"
      return 0
    fi
    if [[ "${status}" == "failed" ]]; then
      echo "${job}" >&2
      return 1
    fi
    sleep 5
  done
  return 1
}

mkdir -p "${WORKDIR}"
cd "${WORKDIR}"

if [[ ! -f photon.ova ]]; then
  log "Downloading Photon OS OVA (~308M)..."
  curl -fL --retry 3 -o photon.ova "${OVA_URL}"
fi

if [[ ! -f "${QCOW}" ]]; then
  log "Extracting OVA and converting to qcow2..."
  rm -rf ova-extract
  mkdir ova-extract
  tar -xf photon.ova -C ova-extract
  VMDK="$(find ova-extract -name '*.vmdk' | head -1)"
  [[ -n "${VMDK}" ]] || { log "No VMDK in OVA"; exit 1; }
  qemu-img convert -p -f vmdk -O qcow2 "${VMDK}" "${QCOW}"
fi

log "Health check..."
curl -sf "${API}/health" | python3 -m json.tool | head -5

log "Import disk from storage..."
IMPORT=$(curl -sf -X POST "${API}/vms/import-from-storage" \
  -H 'Content-Type: application/json' \
  -d "{\"path\":\"e2e-photon/photon.qcow2\"}")
echo "${IMPORT}" | python3 -m json.tool
VM_ID=$(echo "${IMPORT}" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['id'])")

log "GuestKit inspect..."
INS=$(curl -sf -X POST "${API}/vms/${VM_ID}/inspect")
INS_JOB=$(echo "${INS}" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['job_id'])")
INS_RESULT=$(poll_job "${INS_JOB}" inspect)
python3 -c "import sys,json; d=json.load(sys.stdin); r=d.get('data',{}).get('result',{}); assert r.get('inspect') or r.get('data',{}).get('inspect'), r" <<< "${INS_RESULT}"
log "Inspect OK — OS detected"

log "GuestKit doctor..."
DOC=$(curl -sf -X POST "${API}/vms/${VM_ID}/doctor?target=kubevirt&explain=true")
DOC_JOB=$(echo "${DOC}" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['job_id'])")
DOC_RESULT=$(poll_job "${DOC_JOB}" doctor)
python3 -c "import sys,json; d=json.load(sys.stdin); r=d.get('data',{}).get('result',{}).get('data', d.get('data',{}).get('result',{})); assert r.get('bootability') or r.get('boot_report'), r" <<< "${DOC_RESULT}"
log "Doctor OK"

log "Migration plan..."
curl -sf -X POST "${API}/vms/${VM_ID}/migration-plan?target=kubevirt&explain=true" | python3 -m json.tool | head -30

log "Upload qcow2 to MinIO for CDI import..."
OBJECT_KEY="photon-e2e-${TAG}.qcow2"
kubectl -n zyvor exec deploy/minio -- mc mb local/vm-images 2>/dev/null || true
cat "${QCOW}" | kubectl -n zyvor exec -i deploy/minio -- sh -c "cat > /tmp/${OBJECT_KEY} && mc cp /tmp/${OBJECT_KEY} local/vm-images/${OBJECT_KEY} && rm -f /tmp/${OBJECT_KEY}"
kubectl -n zyvor exec deploy/minio -- mc anonymous set download local/vm-images 2>/dev/null || true

log "Provision + apply KubeVirt VM ${VM_KV_NAME}..."
PROV=$(curl -sf -X POST "${API}/vms/${VM_ID}/provision?apply=true")
echo "${PROV}" | python3 -m json.tool | head -40

# Patch generated name if needed — re-apply custom manifest
cat > "/tmp/${VM_KV_NAME}.yaml" <<EOF
apiVersion: cdi.kubevirt.io/v1beta1
kind: DataVolume
metadata:
  name: ${VM_KV_NAME}-disk
  namespace: ${NS}
spec:
  source:
    http:
      url: http://minio.zyvor.svc:9000/vm-images/${OBJECT_KEY}
  pvc:
    accessModes: [ReadWriteOnce]
    resources:
      requests:
        storage: 16Gi
    storageClassName: local-path
---
apiVersion: kubevirt.io/v1
kind: VirtualMachine
metadata:
  name: ${VM_KV_NAME}
  namespace: ${NS}
  labels:
    guestkit.zyvor.dev/e2e: photon
spec:
  runStrategy: Halted
  template:
    metadata:
      labels:
        kubevirt.io/vm: ${VM_KV_NAME}
    spec:
      domain:
        cpu:
          cores: 2
        devices:
          disks:
            - name: rootdisk
              disk:
                bus: virtio
            - name: cloudinitdisk
              disk:
                bus: virtio
          interfaces:
            - name: default
              masquerade: {}
        firmware:
          bootloader:
            efi:
              secureBoot: false
        machine:
          type: q35
        resources:
          requests:
            memory: 2Gi
      networks:
        - name: default
          pod: {}
      volumes:
        - name: rootdisk
          persistentVolumeClaim:
            claimName: ${VM_KV_NAME}-disk
        - name: cloudinitdisk
          cloudInitNoCloud:
            userData: |
              #cloud-config
              password: photon
              chpasswd: { expire: False }
              user: root
              ssh_pwauth: true
              packages:
                - open-vm-tools
                - qemu-guest-agent
              runcmd:
                - systemctl enable --now vmtoolsd || true
                - systemctl enable --now qemu-guest-agent || true
EOF

APPLY=$(curl -sf -X POST "${API}/kubevirt/apply" -H 'Content-Type: application/json' \
  -d "$(python3 -c "import json; print(json.dumps({'yaml': open('/tmp/${VM_KV_NAME}.yaml').read()}))")")
echo "${APPLY}" | python3 -m json.tool

# API may create DataVolume but fail VM apply (RBAC/webhook); ensure VM exists.
if ! kubectl -n "${NS}" get vm "${VM_KV_NAME}" >/dev/null 2>&1; then
  log "Applying VM manifest via kubectl (API apply incomplete)..."
  kubectl apply -f "/tmp/${VM_KV_NAME}.yaml"
fi

log "Starting VM (triggers local-path + CDI import)..."
kubectl -n "${NS}" patch vm "${VM_KV_NAME}" --type merge -p '{"spec":{"runStrategy":"Always"}}' || true

log "Waiting for DataVolume import..."
for i in $(seq 1 120); do
  phase=$(kubectl -n "${NS}" get dv "${VM_KV_NAME}-disk" -o jsonpath='{.status.phase}' 2>/dev/null || echo Pending)
  log "  DV phase ${i}: ${phase}"
  [[ "${phase}" == "Succeeded" ]] && break
  sleep 5
done

for i in $(seq 1 60); do
  phase=$(kubectl -n "${NS}" get vmi "${VM_KV_NAME}" -o jsonpath='{.status.phase}' 2>/dev/null || echo Pending)
  log "  VMI phase ${i}: ${phase}"
  [[ "${phase}" == "Running" ]] && break
  sleep 5
done

log "Guest agent info (live)..."
curl -sf "${API}/kubevirt/vms/${NS}/${VM_KV_NAME}/guest-agent" | python3 -m json.tool | head -25

log "VM Tools install..."
curl -sf -X POST "${API}/kubevirt/vms/${NS}/${VM_KV_NAME}/vmtools/install?restart=false&method=cloud-init" | python3 -m json.tool | head -20 || true

log "VM Tools reconcile..."
curl -sf -X POST "${API}/vmtools/policy/reconcile" | python3 -m json.tool

log "Stopping VM for offline cluster inspect..."
kubectl -n "${NS}" patch vm "${VM_KV_NAME}" --type merge -p '{"spec":{"runStrategy":"Halted"}}' || true
kubectl -n "${NS}" delete vmi "${VM_KV_NAME}" --wait=false 2>/dev/null || true
sleep 15
for i in $(seq 1 30); do
  phase=$(kubectl -n "${NS}" get vmi "${VM_KV_NAME}" -o jsonpath='{.status.phase}' 2>/dev/null || echo Gone)
  log "  stop wait ${i}: ${phase}"
  [[ "${phase}" == "Gone" ]] && break
  kubectl -n "${NS}" delete vmi "${VM_KV_NAME}" --wait=false 2>/dev/null || true
  sleep 3
done

log "Cluster offline inspect..."
CINS=$(curl -sf -X POST "${API}/kubevirt/vms/${NS}/${VM_KV_NAME}/inspect")
CINS_JOB=$(echo "${CINS}" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['job_id'])")
CINS_RESULT=$(poll_job "${CINS_JOB}" cluster-inspect)
python3 -c "import sys,json; d=json.load(sys.stdin); r=d.get('data',{}).get('result',{}); assert r.get('inspect') or r.get('data',{}).get('inspect'), r" <<< "${CINS_RESULT}"

log "Cluster offline doctor..."
CDOC=$(curl -sf -X POST "${API}/kubevirt/vms/${NS}/${VM_KV_NAME}/doctor?target=kubevirt&explain=true")
CDOC_JOB=$(echo "${CDOC}" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['job_id'])")
poll_job "${CDOC_JOB}" cluster-doctor >/dev/null

log "Boot inspect..."
curl -sf -X POST "${API}/kubevirt/vms/${NS}/${VM_KV_NAME}/boot-inspect" | python3 -m json.tool | head -20

log "Coverage..."
curl -sf "${API}/vmtools/coverage" | python3 -m json.tool

log "=== Photon E2E complete: ${NS}/${VM_KV_NAME} ==="
