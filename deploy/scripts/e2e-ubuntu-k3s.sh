#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# End-to-end: Ubuntu cloud image → GuestKit inspect/doctor → KubeVirt VM → live + offline tests.
set -euo pipefail

API="${API:-http://127.0.0.1:30080/api/v1}"
NS="${NS:-zyvor-e2e}"
STORAGE="${STORAGE:-/var/lib/zyvor/images}"
ROOT="${ROOT:-$(cd "$(dirname "$0")/../.." && pwd)}"
WORKDIR="${STORAGE}/e2e-ubuntu"
UBUNTU_URL="${UBUNTU_URL:-https://cloud-images.ubuntu.com/releases/22.04/release/ubuntu-22.04-server-cloudimg-amd64-disk-kvm.img}"
DISK="${WORKDIR}/ubuntu-22.04-kvm.img"
VM_NAME="${VM_NAME:-ubuntu-e2e}"
TAG="$(date +%Y%m%d%H%M%S)"
VM_KV_NAME="${VM_KV_NAME:-${VM_NAME}-${TAG}}"
AGENT_NODE="${AGENT_NODE:-http://127.0.0.1:30092}"
CLUSTER_DNS="${CLUSTER_DNS:-10.43.0.10}"

log() { echo "[e2e-ubuntu] $*" >&2; }

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

publish_agent_to_minio() {
  local agent_bin="${1:-}"
  [[ -f "${agent_bin}" ]] || { log "Agent binary missing: ${agent_bin}"; return 1; }
  log "Publishing guest agent to MinIO..."
  kubectl -n zyvor exec deploy/minio -- sh -c '
    mc alias set local http://127.0.0.1:9000 zyvor zyvor-secret 2>/dev/null || true
    mc mb local/vmtools 2>/dev/null || true
    mc mb local/vm-images 2>/dev/null || true
  '
  cat "${agent_bin}" | kubectl -n zyvor exec -i deploy/minio -- sh -c \
    'cat > /tmp/zyvor-guest-agent && mc cp /tmp/zyvor-guest-agent local/vmtools/linux/zyvor-guest-agent && rm -f /tmp/zyvor-guest-agent'
  kubectl -n zyvor exec deploy/minio -- sh -c \
    'mc anonymous set download local/vmtools 2>/dev/null || true; mc anonymous set download local/vm-images 2>/dev/null || true'
  local code
  code=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:30092/vmtools/linux/zyvor-guest-agent" || echo 000)
  log "Agent URL HTTP ${code}"
  [[ "${code}" == "200" ]]
}

ensure_agent_binary() {
  local musl="${ROOT}/target/x86_64-unknown-linux-musl/release/zyvor-guest-agent"
  local dyn="${ROOT}/target/release/zyvor-guest-agent"
  if [[ -f "${musl}" ]]; then
    printf '%s\n' "${musl}"
    return 0
  fi
  if [[ -f "${dyn}" ]]; then
    printf '%s\n' "${dyn}"
    return 0
  fi
  log "Building zyvor-guest-agent..."
  if (cd "${ROOT}" && cargo build --release -p zyvor-guest-agent >&2); then
    [[ -f "${dyn}" ]] && printf '%s\n' "${dyn}" && return 0
  fi
  return 1
}

publish_qga_deb_to_minio() {
  local deb="${1:-}"
  [[ -f "${deb}" ]] || { log "qemu-guest-agent deb missing: ${deb}"; return 1; }
  log "Publishing qemu-guest-agent deb to MinIO..."
  cat "${deb}" | kubectl -n zyvor exec -i deploy/minio -- sh -c \
    'cat > /tmp/qga.deb && mc cp /tmp/qga.deb local/vmtools/linux/qemu-guest-agent.deb && rm -f /tmp/qga.deb'
  local code
  code=$(curl -s -o /dev/null -w '%{http_code}' "${AGENT_NODE}/vmtools/linux/qemu-guest-agent.deb" || echo 000)
  log "QGA deb URL HTTP ${code}"
  [[ "${code}" == "200" ]]
}

mkdir -p "${WORKDIR}"
cd "${WORKDIR}"

log "Health check..."
curl -sf "${API}/health" | python3 -m json.tool | head -3

AGENT_BIN="$(ensure_agent_binary)"
publish_agent_to_minio "${AGENT_BIN}"

QGA_DEB="${WORKDIR}/qemu-guest-agent.deb"
if [[ ! -f "${QGA_DEB}" ]]; then
  log "Downloading qemu-guest-agent deb..."
  curl -fL --retry 3 -o "${QGA_DEB}" \
    "http://archive.ubuntu.com/ubuntu/pool/universe/q/qemu/qemu-guest-agent_6.2%2bdfsg-2ubuntu6.31_amd64.deb"
fi
publish_qga_deb_to_minio "${QGA_DEB}"

if [[ ! -f "${DISK}" ]]; then
  log "Downloading Ubuntu 22.04 cloud image..."
  curl -fL --retry 3 -o "${DISK}" "${UBUNTU_URL}"
fi
log "Disk: $(du -h "${DISK}" | awk '{print $1}')"

log "Import disk via API upload..."
IMPORT=$(curl -sf -F "file=@${DISK}" "${API}/vms/import")
echo "${IMPORT}" | python3 -m json.tool | head -12
VM_ID=$(echo "${IMPORT}" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['id'])")

log "GuestKit inspect..."
INS=$(curl -sf -X POST "${API}/vms/${VM_ID}/inspect")
INS_JOB=$(echo "${INS}" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['job_id'])")
INS_RESULT=$(poll_job "${INS_JOB}" inspect)
python3 -c "import sys,json; d=json.load(sys.stdin); r=d.get('data',{}).get('result',{}); assert r.get('inspect') or r.get('data',{}).get('inspect'), r" <<< "${INS_RESULT}"
python3 -c "
import sys,json
d=json.load(sys.stdin)
r=d.get('data',{}).get('result',{})
ev=r.get('inspect') or r.get('data',{}).get('inspect') or {}
os=ev.get('os') or {}
print('OS:', os.get('os_type'), os.get('distribution'), os.get('version'))
" <<< "${INS_RESULT}"

log "GuestKit doctor..."
DOC=$(curl -sf -X POST "${API}/vms/${VM_ID}/doctor?target=kubevirt&explain=true")
DOC_JOB=$(echo "${DOC}" | python3 -c "import sys,json; print(json.load(sys.stdin)['data']['job_id'])")
DOC_RESULT=$(poll_job "${DOC_JOB}" doctor)
python3 -c "import sys,json; d=json.load(sys.stdin); r=d.get('data',{}).get('result',{}).get('data', d.get('data',{}).get('result',{})); assert r.get('bootability') or r.get('boot_report'), r" <<< "${DOC_RESULT}"
log "Doctor OK"

log "Upload disk to MinIO for CDI..."
OBJECT_KEY="ubuntu-e2e-${TAG}.img"
cat "${DISK}" | kubectl -n zyvor exec -i deploy/minio -- sh -c \
  "cat > /tmp/${OBJECT_KEY} && mc cp /tmp/${OBJECT_KEY} local/vm-images/${OBJECT_KEY} && rm -f /tmp/${OBJECT_KEY}"

kubectl create namespace "${NS}" --dry-run=client -o yaml | kubectl apply -f -

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
        storage: 12Gi
    storageClassName: local-path
---
apiVersion: kubevirt.io/v1
kind: VirtualMachine
metadata:
  name: ${VM_KV_NAME}
  namespace: ${NS}
  labels:
    guestkit.zyvor.dev/e2e: ubuntu
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
            - name: guestagent
              disk:
                bus: virtio
              serial: org.qemu.guest_agent.0
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
        - name: guestagent
          emptyDisk:
            capacity: 1Gi
        - name: cloudinitdisk
          cloudInitNoCloud:
            userData: |
              #cloud-config
              users:
                - default
                - name: ubuntu
                  sudo: ALL=(ALL) NOPASSWD:ALL
                  shell: /bin/bash
              password: ubuntu
              chpasswd: { expire: False }
              ssh_pwauth: true
              write_files:
                - path: /etc/systemd/system/zyvor-guest-agent.service
                  permissions: "0644"
                  content: |
                    [Unit]
                    Description=Zyvor Guest Agent
                    After=network-online.target qemu-guest-agent.service
                    Wants=network-online.target
                    ConditionPathExists=/dev/virtio-ports/org.qemu.guest_agent.0
                    [Service]
                    ExecStart=/usr/local/bin/zyvor-guest-agent
                    Restart=always
                    RestartSec=5
                    User=root
                    [Install]
                    WantedBy=multi-user.target
              runcmd:
                - curl -fsSL -o /tmp/qga.deb http://${AGENT_NODE}/vmtools/linux/qemu-guest-agent.deb
                - dpkg -i /tmp/qga.deb
                - systemctl enable --now qemu-guest-agent
                - curl -fsSL -o /usr/local/bin/zyvor-guest-agent http://${AGENT_NODE}/vmtools/linux/zyvor-guest-agent
                - chmod 755 /usr/local/bin/zyvor-guest-agent
                - systemctl daemon-reload
                - systemctl enable --now zyvor-guest-agent
EOF

log "Applying KubeVirt manifests..."
kubectl apply -f "/tmp/${VM_KV_NAME}.yaml"

log "Starting VM..."
kubectl -n "${NS}" patch vm "${VM_KV_NAME}" --type merge -p '{"spec":{"runStrategy":"Always"}}'

log "Waiting for DataVolume import..."
for i in $(seq 1 120); do
  phase=$(kubectl -n "${NS}" get dv "${VM_KV_NAME}-disk" -o jsonpath='{.status.phase}' 2>/dev/null || echo Pending)
  log "  DV phase ${i}: ${phase}"
  [[ "${phase}" == "Succeeded" ]] && break
  sleep 5
done

for i in $(seq 1 90); do
  phase=$(kubectl -n "${NS}" get vmi "${VM_KV_NAME}" -o jsonpath='{.status.phase}' 2>/dev/null || echo Pending)
  log "  VMI phase ${i}: ${phase}"
  [[ "${phase}" == "Running" ]] && break
  sleep 5
done

log "Guest agent status..."
curl -sf "${API}/kubevirt/vms/${NS}/${VM_KV_NAME}/guest-agent" | python3 -m json.tool | head -25

log "Guest Control Fabric — status + doctor..."
curl -sf "${API}/kubevirt/vms/${NS}/${VM_KV_NAME}/guest/status" | python3 -m json.tool | head -40 || true
curl -sf "${API}/kubevirt/vms/${NS}/${VM_KV_NAME}/guest/doctor" | python3 -m json.tool | head -50 || true
curl -sf "${API}/kubevirt/vms/${NS}/${VM_KV_NAME}/guest/capabilities" | python3 -m json.tool | head -35 || true

log "Guest Control Fabric — airgap QGA file bootstrap install (when guest network unavailable)..."
curl -sf -X POST "${API}/kubevirt/vms/${NS}/${VM_KV_NAME}/guest/install-agent" \
  -H 'Content-Type: application/json' \
  -d '{"strategy":"auto","restart":false}' | python3 -m json.tool | head -35 || true

log "VM Tools install (cloud-init merge + agent)..."
curl -sf -X POST "${API}/kubevirt/vms/${NS}/${VM_KV_NAME}/vmtools/install?restart=true&method=cloud-init" | python3 -m json.tool | head -30 || true

log "Waiting for guest agent after restart..."
for i in $(seq 1 60); do
  connected=$(curl -sf "${API}/kubevirt/vms" | python3 -c "
import sys,json
for v in json.load(sys.stdin).get('data',[]):
  if v.get('namespace')=='${NS}' and v.get('name')=='${VM_KV_NAME}':
    print('1' if v.get('guest_agent_connected') else '0')
    break
" 2>/dev/null || echo 0)
  log "  agent connected poll ${i}: ${connected}"
  [[ "${connected}" == "1" ]] && break
  sleep 10
done

log "Live guest intel..."
curl -sf "${API}/kubevirt/vms/${NS}/${VM_KV_NAME}/guest/info" | python3 -m json.tool | head -30 || true
curl -s "${API}/kubevirt/vms/${NS}/${VM_KV_NAME}/guest/evidence" | python3 -m json.tool | head -35 || log "guest/evidence not ready"
curl -s "${API}/kubevirt/vms/${NS}/${VM_KV_NAME}/guest/network" | python3 -m json.tool | head -20 || true
curl -sf "${API}/kubevirt/vms/${NS}/${VM_KV_NAME}/guest/health" | python3 -m json.tool | head -15 || true
curl -sf "${API}/kubevirt/vms/${NS}/${VM_KV_NAME}/guest/journal?limit=5" 2>/dev/null | python3 -m json.tool | head -15 || true

log "Stopping VM for offline cluster inspect..."
kubectl -n "${NS}" patch vm "${VM_KV_NAME}" --type merge -p '{"spec":{"runStrategy":"Halted"}}' || true
kubectl -n "${NS}" delete vmi "${VM_KV_NAME}" --wait=false 2>/dev/null || true
sleep 15
for i in $(seq 1 30); do
  phase=$(kubectl -n "${NS}" get vmi "${VM_KV_NAME}" -o jsonpath='{.status.phase}' 2>/dev/null || echo Gone)
  log "  stop wait ${i}: ${phase}"
  [[ "${phase}" == "Gone" ]] && break
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
curl -sf -X POST "${API}/kubevirt/vms/${NS}/${VM_KV_NAME}/boot-inspect" | python3 -m json.tool | head -25 || true

log "=== Ubuntu E2E complete: ${NS}/${VM_KV_NAME} ==="
