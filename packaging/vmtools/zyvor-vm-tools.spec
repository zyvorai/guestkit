Name:           zyvor-vm-tools
Version:        0.1.0
Release:        1%{?dist}
Summary:        Zyvor GuestKit Agent — in-guest VM intelligence and migration assurance

License:        Apache-2.0
URL:            https://zyvor.dev/guestkit
Source0:        zyvor-vm-tools-%{version}.tar.gz
BuildRequires:  rust >= 1.70
BuildRequires:  cargo
BuildRequires:  gcc
BuildRequires:  musl-gcc

Requires:       systemd
Provides:       zyvor-guest-agent = %{version}
Provides:       guestkit-agent = %{version}

%description
The Zyvor GuestKit Agent (guestkitd) provides in-guest inventory, heartbeat,
performance telemetry, migration readiness assessment, repair planning, and
snapshot coordination for KVM/KubeVirt VMs. Includes the guestkitctl local
troubleshooting CLI and the guestkitd-exec privileged helper.

%prep
%autosetup -n guestkit-%{version} -p1

%build
export CARGO_TARGET_DIR=target
rustup target add x86_64-unknown-linux-musl 2>/dev/null || true
cargo build --release --target x86_64-unknown-linux-musl \
  -p zyvor-guest-agent

%install
install -Dm755 target/x86_64-unknown-linux-musl/release/guestkitd \
  %{buildroot}%{_bindir}/guestkitd
install -Dm755 target/x86_64-unknown-linux-musl/release/guestkitd-exec \
  %{buildroot}%{_bindir}/guestkitd-exec
install -Dm755 target/x86_64-unknown-linux-musl/release/guestkitctl \
  %{buildroot}%{_bindir}/guestkitctl
# Compatibility names for pre-rebrand tooling
ln -s guestkitd %{buildroot}%{_bindir}/zyvor-guest-agent
ln -s guestkitd-exec %{buildroot}%{_bindir}/zyvor-guest-agent-exec
install -Dm644 templates/agent/guestkit-agent.service \
  %{buildroot}%{_unitdir}/guestkit-agent.service
install -Dm644 templates/agent/zyvor-guest-agent-exec.service \
  %{buildroot}%{_unitdir}/zyvor-guest-agent-exec.service
install -Dm644 templates/agent/agent-policy.yaml \
  %{buildroot}%{_sysconfdir}/guestkit/agent-policy.yaml
install -Dm644 templates/agent/60-zyvor-guest-agent.rules \
  %{buildroot}%{_udevrulesdir}/60-zyvor-guest-agent.rules

%pre
getent group zyvor-agent >/dev/null || groupadd -r zyvor-agent
getent passwd zyvor-agent >/dev/null || \
  useradd -r -g zyvor-agent -s /sbin/nologin -d /var/lib/guestkit zyvor-agent
exit 0

%post
# Upgrade path: retire the pre-rebrand unit if it was actually installed.
if [ -f %{_unitdir}/zyvor-guest-agent.service ]; then
  systemctl disable --now zyvor-guest-agent.service >/dev/null 2>&1 || :
  rm -f %{_unitdir}/zyvor-guest-agent.service
fi
# Hand the hypervisor channel to the (now existing) zyvor-agent group.
udevadm control --reload >/dev/null 2>&1 || :
udevadm trigger --subsystem-match=virtio-ports --action=change >/dev/null 2>&1 || :
%systemd_post guestkit-agent.service

%preun
%systemd_preun guestkit-agent.service

%postun
%systemd_postun_with_restart guestkit-agent.service

%files
%{_bindir}/guestkitd
%{_bindir}/guestkitd-exec
%{_bindir}/guestkitctl
%{_bindir}/zyvor-guest-agent
%{_bindir}/zyvor-guest-agent-exec
%{_unitdir}/guestkit-agent.service
%{_unitdir}/zyvor-guest-agent-exec.service
%{_udevrulesdir}/60-zyvor-guest-agent.rules
%config(noreplace) %{_sysconfdir}/guestkit/agent-policy.yaml

%changelog
* Sat Jul 18 2026 ZyvorAI Labs <info@zyvor.dev> - 0.1.0-2
- Rebrand to GuestKit Agent: guestkitd, guestkitd-exec, guestkitctl
- guestkit-agent.service (hardened) with zyvor-guest-agent.service alias
- Compatibility symlinks and Provides for pre-rebrand names

* Fri Jun 12 2026 ZyvorAI Labs <info@zyvor.dev> - 0.1.0-1
- Initial Zeus VM Tools Linux package
