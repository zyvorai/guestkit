#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Build zyvor-vm-tools release artifacts (tar.gz, deb, rpm, iso).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
VERSION="${VERSION:-0.1.0}"
DIST="${ROOT}/dist/vmtools"
ISO_DIR="${DIST}/iso-root"
DEB_ROOT="${DIST}/deb-root"

echo "Building guestkitd (musl)..."
cd "${ROOT}"
rustup target add x86_64-unknown-linux-musl 2>/dev/null || true
cargo build --release -p zyvor-guest-agent \
  --target x86_64-unknown-linux-musl

mkdir -p "${DIST}/linux" "${ISO_DIR}/linux"
cp "target/x86_64-unknown-linux-musl/release/guestkitd" "${DIST}/linux/guestkitd"
cp "target/x86_64-unknown-linux-musl/release/guestkitd-exec" "${DIST}/linux/guestkitd-exec"
cp "target/x86_64-unknown-linux-musl/release/guestkitctl" "${DIST}/linux/guestkitctl"
# Compatibility artifact names for pre-rebrand updaters/installers
cp "${DIST}/linux/guestkitd" "${DIST}/linux/zyvor-guest-agent"
cp "${DIST}/linux/guestkitd-exec" "${DIST}/linux/zyvor-guest-agent-exec"
cp templates/agent/guestkit-agent.service "${DIST}/linux/"
cp templates/agent/zyvor-guest-agent-exec.service "${DIST}/linux/"
cp templates/agent/zyvor-guest-updater.service "${DIST}/linux/"
cp templates/agent/zyvor-guest-updater.timer "${DIST}/linux/"
cp templates/agent/agent-policy.yaml "${DIST}/linux/"
cp templates/agent/guest-agent.toml "${DIST}/linux/"
mkdir -p "${DIST}/linux/hooks/pre-snapshot"
cp templates/agent/hooks/pre-snapshot/*.sh "${DIST}/linux/hooks/pre-snapshot/"
chmod 755 "${DIST}/linux/hooks/pre-snapshot/"*.sh
tar czf "${DIST}/linux/zyvor-vm-tools-linux-amd64.tar.gz" \
  -C "${DIST}/linux" guestkitd guestkitd-exec \
  guestkit-agent.service zyvor-guest-agent-exec.service \
  zyvor-guest-updater.service zyvor-guest-updater.timer \
  agent-policy.yaml guest-agent.toml hooks
LINUX_TAR_SHA256="$(sha256sum "${DIST}/linux/zyvor-vm-tools-linux-amd64.tar.gz" | awk '{print $1}')"
echo "${LINUX_TAR_SHA256}" > "${DIST}/linux/zyvor-vm-tools-linux-amd64.tar.gz.sha256"
MANIFEST_JSON=$(printf '{"version":"%s","channel":"stable","linux_tar_sha256":"%s"}' "$VERSION" "$LINUX_TAR_SHA256")
LINUX_TAR_SIGNATURE=$(cargo run -q -p zyvor-guest-agent -- sign-manifest "${MANIFEST_JSON}" 2>/dev/null || true)
if [ -n "${LINUX_TAR_SIGNATURE}" ]; then
  echo "${LINUX_TAR_SIGNATURE}" > "${DIST}/linux/zyvor-vm-tools-linux-amd64.sig"
fi

echo "Building DEB..."
rm -rf "${DEB_ROOT}"
mkdir -p "${DEB_ROOT}/DEBIAN" "${DEB_ROOT}/usr/bin" "${DEB_ROOT}/lib/systemd/system" "${DEB_ROOT}/etc/zyvor"
cp "${DIST}/linux/guestkitd" "${DEB_ROOT}/usr/bin/"
cp "${DIST}/linux/guestkitd-exec" "${DEB_ROOT}/usr/bin/"
cp "${DIST}/linux/guestkit-agent.service" "${DEB_ROOT}/lib/systemd/system/"
cp "${DIST}/linux/zyvor-guest-agent-exec.service" "${DEB_ROOT}/lib/systemd/system/"
cp "${DIST}/linux/zyvor-guest-updater.service" "${DEB_ROOT}/lib/systemd/system/"
cp "${DIST}/linux/zyvor-guest-updater.timer" "${DEB_ROOT}/lib/systemd/system/"
cp "${DIST}/linux/agent-policy.yaml" "${DEB_ROOT}/etc/zyvor/"
cp "${DIST}/linux/guest-agent.toml" "${DEB_ROOT}/etc/zyvor/"
mkdir -p "${DEB_ROOT}/etc/zyvor/hooks/pre-snapshot"
cp "${DIST}/linux/hooks/pre-snapshot/"*.sh "${DEB_ROOT}/etc/zyvor/hooks/pre-snapshot/"
chmod 755 "${DEB_ROOT}/etc/zyvor/hooks/pre-snapshot/"*.sh
cat > "${DEB_ROOT}/DEBIAN/control" <<EOF
Package: zyvor-vm-tools
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: amd64
Maintainer: ZyvorAI Labs <info@zyvor.dev>
Description: Zeus VM Tools — Zyvor in-guest agent for KubeVirt VMs
Depends: systemd, libsystemd0
EOF
cat > "${DEB_ROOT}/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
if ! getent group zyvor-agent >/dev/null 2>&1; then
  addgroup --system zyvor-agent 2>/dev/null || groupadd -r zyvor-agent 2>/dev/null || true
fi
if ! getent passwd zyvor-agent >/dev/null 2>&1; then
  adduser --system --ingroup zyvor-agent --home /var/lib/zyvor --shell /usr/sbin/nologin zyvor-agent 2>/dev/null || \
    useradd -r -g zyvor-agent -d /var/lib/zyvor -s /sbin/nologin zyvor-agent 2>/dev/null || true
fi
mkdir -p /var/lib/zyvor /etc/zyvor
chown zyvor-agent:zyvor-agent /var/lib/zyvor 2>/dev/null || true
chmod 750 /var/lib/zyvor 2>/dev/null || true
systemctl daemon-reload || true
systemctl enable guestkit-agent.service || true
systemctl enable zyvor-guest-agent-exec.service || true
systemctl enable zyvor-guest-updater.timer || true
systemctl start zyvor-guest-agent-exec.service || true
EOF
chmod 755 "${DEB_ROOT}/DEBIAN/postinst"
dpkg-deb --build "${DEB_ROOT}" "${DIST}/linux/zyvor-vm-tools_${VERSION}_amd64.deb"

if command -v rpmbuild >/dev/null; then
  echo "Building RPM..."
  RPM_TOP="${DIST}/rpm"
  mkdir -p "${RPM_TOP}"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
  tar czf "${RPM_TOP}/SOURCES/zyvor-vm-tools-${VERSION}.tar.gz" \
    -C "${ROOT}" \
    --transform "s,^,guestkit-${VERSION}/," \
    templates/agent/guestkit-agent.service \
    target/x86_64-unknown-linux-musl/release/guestkitd \
    target/x86_64-unknown-linux-musl/release/guestkitd-exec \
    target/x86_64-unknown-linux-musl/release/guestkitctl \
    templates/agent/zyvor-guest-agent-exec.service \
    templates/agent/agent-policy.yaml
  sed "s/^Version:.*/Version:        ${VERSION}/" "${ROOT}/packaging/vmtools/zyvor-vm-tools.spec" \
    > "${RPM_TOP}/SPECS/zyvor-vm-tools.spec"
  rpmbuild -bb --define "_topdir ${RPM_TOP}" "${RPM_TOP}/SPECS/zyvor-vm-tools.spec" || true
  find "${RPM_TOP}/RPMS" -name '*.rpm' -exec cp {} "${DIST}/linux/zyvor-vm-tools-${VERSION}.rpm" \; 2>/dev/null || true
fi

cat > "${ISO_DIR}/linux/install.sh" <<'EOF'
#!/bin/sh
set -eu
ARCH="$(uname -m)"
if [ -f /etc/redhat-release ]; then
  rpm -Uvh --force linux/zyvor-vm-tools-*.rpm 2>/dev/null || cp linux/guestkitd /usr/bin/guestkitd
elif [ -f /etc/debian_version ]; then
  dpkg -i linux/zyvor-vm-tools_*.deb 2>/dev/null || cp linux/guestkitd /usr/bin/guestkitd
else
  cp linux/guestkitd /usr/bin/guestkitd
fi
chmod 755 /usr/bin/guestkitd
chmod 755 /usr/bin/guestkitd-exec 2>/dev/null || true
ln -sf guestkitd /usr/bin/zyvor-guest-agent
ln -sf guestkitd-exec /usr/bin/zyvor-guest-agent-exec
install -Dm644 linux/guestkit-agent.service /etc/systemd/system/guestkit-agent.service
install -Dm644 linux/zyvor-guest-agent-exec.service /etc/systemd/system/zyvor-guest-agent-exec.service
install -Dm644 linux/zyvor-guest-updater.service /etc/systemd/system/zyvor-guest-updater.service
install -Dm644 linux/zyvor-guest-updater.timer /etc/systemd/system/zyvor-guest-updater.timer
install -Dm644 linux/agent-policy.yaml /etc/zyvor/agent-policy.yaml
install -Dm644 linux/guest-agent.toml /etc/zyvor/guest-agent.toml
mkdir -p /etc/zyvor/hooks/pre-snapshot
install -m755 linux/hooks/pre-snapshot/*.sh /etc/zyvor/hooks/pre-snapshot/ 2>/dev/null || true
if ! getent group zyvor-agent >/dev/null 2>&1; then
  addgroup --system zyvor-agent 2>/dev/null || groupadd -r zyvor-agent 2>/dev/null || true
fi
if ! getent passwd zyvor-agent >/dev/null 2>&1; then
  adduser --system --ingroup zyvor-agent --home /var/lib/zyvor --shell /usr/sbin/nologin zyvor-agent 2>/dev/null || \
    useradd -r -g zyvor-agent -d /var/lib/zyvor -s /sbin/nologin zyvor-agent 2>/dev/null || true
fi
mkdir -p /var/lib/zyvor
chown zyvor-agent:zyvor-agent /var/lib/zyvor 2>/dev/null || true
chmod 750 /var/lib/zyvor 2>/dev/null || true
systemctl daemon-reload
systemctl enable --now zyvor-guest-agent-exec
systemctl enable --now zyvor-guest-agent
systemctl enable --now zyvor-guest-updater.timer
echo "Zyvor VM Tools installed"
EOF
chmod +x "${ISO_DIR}/linux/install.sh"
cp "${DIST}/linux/guestkitd" "${ISO_DIR}/linux/"
cp "${DIST}/linux/guestkit-agent.service" "${ISO_DIR}/linux/"
cp "${DIST}/linux/zyvor-vm-tools_${VERSION}_amd64.deb" "${ISO_DIR}/linux/" 2>/dev/null || true
cp "${DIST}/linux/zyvor-vm-tools-${VERSION}.rpm" "${ISO_DIR}/linux/" 2>/dev/null || true

cat > "${ISO_DIR}/manifest.json" <<EOF
{"version":"${VERSION}","product":"Zeus VM Tools","platforms":["linux-amd64","windows-amd64"],"linuxTarSha256":"${LINUX_TAR_SHA256}","linuxTarSignature":"${LINUX_TAR_SIGNATURE}"}
EOF

if command -v genisoimage >/dev/null; then
  genisoimage -o "${DIST}/zyvor-vm-tools.iso" -V ZYVOR_VM_TOOLS -r "${ISO_DIR}"
  echo "ISO: ${DIST}/zyvor-vm-tools.iso"
elif command -v mkisofs >/dev/null; then
  mkisofs -o "${DIST}/zyvor-vm-tools.iso" -V ZYVOR_VM_TOOLS -r "${ISO_DIR}"
  echo "ISO: ${DIST}/zyvor-vm-tools.iso"
fi

echo "Artifacts in ${DIST}/linux"
ls -la "${DIST}/linux"

echo "Building Windows artifacts..."
bash "${ROOT}/packaging/vmtools/windows/build-msi.sh"
mkdir -p "${DIST}/windows"
cp -a "${ROOT}/dist/vmtools/windows/"* "${DIST}/windows/" 2>/dev/null || true
