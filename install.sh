#!/bin/bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Guestkit Worker Installation Script
# Version: 0.1.0
#

set -e

BINARY="target/release/guestkit-worker"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
SYSTEMD_DIR="${SYSTEMD_DIR:-/etc/systemd/system}"

echo "========================================"
echo " Guestkit Worker Installation"
echo " Version: 0.1.0"
echo "========================================"
echo

# Check if binary exists
if [ ! -f "$BINARY" ]; then
    echo "❌ Error: Binary not found at $BINARY"
    echo "Please build first: cargo build --release"
    exit 1
fi

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo "⚠️  Warning: Not running as root. Will attempt with sudo."
    SUDO="sudo"
else
    SUDO=""
fi

# Install binary
echo "📦 Installing binary to $INSTALL_DIR..."
$SUDO cp -v "$BINARY" "$INSTALL_DIR/guestkit-worker"
$SUDO chmod +x "$INSTALL_DIR/guestkit-worker"

# Verify installation
echo
echo "✓ Verifying installation..."
if command -v guestkit-worker >/dev/null 2>&1; then
    guestkit-worker --version
    echo "✓ Installation successful!"
else
    echo "❌ Warning: guestkit-worker not found in PATH"
    echo "You may need to add $INSTALL_DIR to your PATH"
fi

# Optionally install systemd service
echo
read -p "Install systemd service? (y/N) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    cat > /tmp/guestkit-worker.service <<'EOF'
[Unit]
Description=Guestkit Worker Daemon
After=network.target

[Service]
Type=simple
User=nobody
Group=nobody
ExecStart=/usr/local/bin/guestkit-worker daemon --transport http
Restart=always
RestartSec=10

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/tmp/guestkit-worker

[Install]
WantedBy=multi-user.target
EOF

    $SUDO cp /tmp/guestkit-worker.service "$SYSTEMD_DIR/guestkit-worker.service"
    $SUDO systemctl daemon-reload

    echo "✓ Systemd service installed"
    echo
    echo "To enable and start:"
    echo "  sudo systemctl enable guestkit-worker"
    echo "  sudo systemctl start guestkit-worker"
    echo
    echo "To check status:"
    echo "  sudo systemctl status guestkit-worker"
fi

echo
echo "========================================"
echo " Installation Complete!"
echo "========================================"
echo
echo "Quick start:"
echo "  guestkit-worker daemon --transport http"
echo
echo "Submit a job:"
echo "  guestkit-worker submit -o system.echo -j '{\"message\":\"hello\"}'"
echo
echo "Documentation:"
echo "  docs/INDEX.md"
echo "  docs/user-guides/quick-reference.md"
echo
