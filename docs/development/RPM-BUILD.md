# Building GuestKit RPM Packages

This guide explains how to build RPM packages for GuestKit.

## 📦 Available Spec Files

### `guestkit.spec` (Basic)
- Standard RPM package
- Rust binary only
- Minimal dependencies
- Recommended for most users

### `guestkit-full.spec` (Full-Featured)
- Includes Python bindings (optional via bcond)
- Shell completions support
- Development package
- More comprehensive

## 🚀 Quick Start

### Prerequisites

**Fedora/RHEL:**
```bash
sudo dnf install -y rpm-build rpmdevtools rust cargo gcc pkgconfig systemd-devel
```

**For Python bindings (guestkit-full.spec):**
```bash
sudo dnf install -y python3-devel python3-pip python3-maturin
```

### Setup RPM Build Environment

```bash
# Create RPM build tree
rpmdev-setuptree

# This creates:
# ~/rpmbuild/
# ├── BUILD/
# ├── RPMS/
# ├── SOURCES/
# ├── SPECS/
# └── SRPMS/
```

## 🔨 Building from Git

### Method 1: Using Basic Spec File

```bash
# Clone the repository
git clone https://github.com/zyvorai/guestkit
cd guestkit

# Create source tarball
git archive --format=tar.gz --prefix=guestkit-1.0.1/ -o ~/rpmbuild/SOURCES/guestkit-1.0.1.tar.gz HEAD

# Copy spec file
cp guestkit.spec ~/rpmbuild/SPECS/

# Build RPM
cd ~/rpmbuild
rpmbuild -ba SPECS/guestkit.spec
```

### Method 2: Using Full-Featured Spec File

```bash
# Clone the repository
git clone https://github.com/zyvorai/guestkit
cd guestkit

# Create source tarball
git archive --format=tar.gz --prefix=guestkit-1.0.1/ -o ~/rpmbuild/SOURCES/guestkit-1.0.1.tar.gz HEAD

# Copy spec file
cp guestkit-full.spec ~/rpmbuild/SPECS/

# Build with Python bindings
rpmbuild -ba SPECS/guestkit-full.spec

# Or build without Python bindings
rpmbuild -ba --without python SPECS/guestkit-full.spec
```

## 📥 Building from Release Tarball

```bash
# Download release tarball
wget https://github.com/zyvorai/guestkit/archive/v1.2.4/guestkit-1.2.4.tar.gz \
     -O ~/rpmbuild/SOURCES/guestkit-1.2.4.tar.gz

# Copy spec file
cp guestkit.spec ~/rpmbuild/SPECS/

# Build
rpmbuild -ba ~/rpmbuild/SPECS/guestkit.spec
```

## 🎯 Build Options

### Build Binary RPM Only

```bash
rpmbuild -bb SPECS/guestkit.spec
```

### Build Source RPM Only

```bash
rpmbuild -bs SPECS/guestkit.spec
```

### Build with Mock (Clean Room)

```bash
# Install mock
sudo dnf install -y mock

# Add yourself to mock group
sudo usermod -aG mock $USER
newgrp mock

# Build SRPM first
rpmbuild -bs SPECS/guestkit.spec

# Build with mock
mock -r fedora-39-x86_64 SRPMS/guestkit-1.0.1-1.fc39.src.rpm
```

## 📦 Generated Packages

After successful build, packages will be in:

```
~/rpmbuild/RPMS/x86_64/
├── guestkit-1.0.1-1.fc39.x86_64.rpm          # Main package
├── guestkit-devel-1.0.1-1.fc39.x86_64.rpm    # Development files
└── python3-guestkit-1.0.1-1.fc39.x86_64.rpm  # Python bindings (full spec only)

~/rpmbuild/SRPMS/
└── guestkit-1.0.1-1.fc39.src.rpm             # Source RPM
```

## ✅ Installing Built Packages

### Install Main Package

```bash
sudo dnf install ~/rpmbuild/RPMS/x86_64/guestkit-1.0.1-1.fc39.x86_64.rpm
```

### Install with Python Bindings

```bash
sudo dnf install ~/rpmbuild/RPMS/x86_64/guestkit-1.0.1-1.fc39.x86_64.rpm \
                 ~/rpmbuild/RPMS/x86_64/python3-guestkit-1.0.1-1.fc39.x86_64.rpm
```

### Install Development Package

```bash
sudo dnf install ~/rpmbuild/RPMS/x86_64/guestkit-devel-1.0.1-1.fc39.x86_64.rpm
```

## 🧪 Testing the Package

```bash
# Check version
guestkit --version

# Run inspection
guestkit inspect /path/to/vm.qcow2

# Launch TUI
guestkit tui /path/to/vm.qcow2

# Test Python bindings (if installed)
python3 -c "from guestkit import Guestfs; print('OK')"
```

## 🔧 Troubleshooting

### Missing Dependencies

If build fails with missing dependencies:

```bash
# Install all build dependencies
sudo dnf builddep guestkit.spec
```

### Rust Version Too Old

```bash
# Check Rust version
rustc --version

# Update Rust
rustup update stable

# Or install from Fedora repos
sudo dnf install rust cargo
```

### Python Bindings Build Failure

```bash
# Install maturin
pip3 install --user maturin

# Or from Fedora
sudo dnf install python3-maturin
```

### Cargo Network Issues

If cargo fails to fetch dependencies:

```bash
# Use vendored dependencies (advanced)
cargo vendor
tar czf ~/rpmbuild/SOURCES/guestkit-vendor-1.0.1.tar.gz vendor/
```

## 📋 RPM Query Commands

### List Package Contents

```bash
rpm -qlp ~/rpmbuild/RPMS/x86_64/guestkit-1.0.1-1.fc39.x86_64.rpm
```

### Check Package Information

```bash
rpm -qip ~/rpmbuild/RPMS/x86_64/guestkit-1.0.1-1.fc39.x86_64.rpm
```

### Verify Package

```bash
rpm -Vp ~/rpmbuild/RPMS/x86_64/guestkit-1.0.1-1.fc39.x86_64.rpm
```

### Check Dependencies

```bash
rpm -qRp ~/rpmbuild/RPMS/x86_64/guestkit-1.0.1-1.fc39.x86_64.rpm
```

## 🌐 Building for Different Architectures

### x86_64 (default)

```bash
rpmbuild -ba --target x86_64 SPECS/guestkit.spec
```

### aarch64 (ARM64)

```bash
rpmbuild -ba --target aarch64 SPECS/guestkit.spec
```

### ppc64le (POWER)

```bash
rpmbuild -ba --target ppc64le SPECS/guestkit.spec
```

## 📤 Distributing Packages

### Create Repository

```bash
# Install createrepo
sudo dnf install -y createrepo_c

# Create repo directory
mkdir -p ~/guestkit-repo

# Copy RPMs
cp ~/rpmbuild/RPMS/x86_64/*.rpm ~/guestkit-repo/

# Create repo metadata
createrepo_c ~/guestkit-repo/

# Serve via HTTP (for testing)
cd ~/guestkit-repo
python3 -m http.server 8000
```

### COPR (Fedora Build Service)

```bash
# Install copr-cli
sudo dnf install -y copr-cli

# Configure copr-cli (follow prompts)
copr-cli --help

# Create COPR project
copr-cli create guestkit --chroot fedora-39-x86_64 --chroot fedora-40-x86_64

# Submit build
copr-cli build guestkit ~/rpmbuild/SRPMS/guestkit-1.0.1-1.fc39.src.rpm
```

## 🔍 Spec File Validation

### Check for rpmlint issues

```bash
# Install rpmlint
sudo dnf install -y rpmlint

# Check spec file
rpmlint guestkit.spec

# Check built RPM
rpmlint ~/rpmbuild/RPMS/x86_64/guestkit-1.0.1-1.fc39.x86_64.rpm
```

## 📚 Additional Resources

- [Fedora Packaging Guidelines](https://docs.fedoraproject.org/en-US/packaging-guidelines/)
- [RPM Packaging Guide](https://rpm-packaging-guide.github.io/)
- [Rust Packaging Guidelines](https://docs.fedoraproject.org/en-US/packaging-guidelines/Rust/)
- [COPR Documentation](https://docs.pagure.org/copr.copr/)

## 🆘 Getting Help

- **Issues**: https://github.com/zyvorai/guestkit/issues
- **Discussions**: https://github.com/zyvorai/guestkit/discussions
- **Email**: ssahani@zyvor.dev

---

**Last Updated**: 2026-01-27
