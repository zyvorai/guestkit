#!/bin/bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Validation script for Docker setup (no build required)

set -e

echo "🔍 Validating Docker setup for guestkit..."
echo

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Check 1: Docker installed
echo -n "Checking if Docker is installed... "
if command -v docker &> /dev/null; then
    echo -e "${GREEN}✓${NC}"
    docker --version
else
    echo -e "${RED}✗${NC}"
    echo "Docker is not installed. Please install Docker first."
    exit 1
fi

# Check 2: Docker service running
echo -n "Checking if Docker service is running... "
if systemctl is-active docker &> /dev/null || pgrep dockerd &> /dev/null; then
    echo -e "${GREEN}✓${NC}"
else
    echo -e "${RED}✗${NC}"
    echo "Docker service is not running. Start it with: sudo systemctl start docker"
    exit 1
fi

# Check 3: User permissions
echo -n "Checking Docker permissions... "
if groups | grep -q docker; then
    echo -e "${GREEN}✓${NC} (user in docker group)"
    CAN_RUN_DOCKER="direct"
elif [ "$EUID" -eq 0 ]; then
    echo -e "${GREEN}✓${NC} (running as root)"
    CAN_RUN_DOCKER="direct"
elif sudo -n docker ps &> /dev/null; then
    echo -e "${YELLOW}⚠${NC} (requires sudo)"
    CAN_RUN_DOCKER="sudo"
else
    echo -e "${YELLOW}⚠${NC} (limited access)"
    echo "  You'll need to use 'sudo docker' for Docker commands"
    echo "  Or add yourself to docker group: sudo usermod -aG docker \$USER"
    CAN_RUN_DOCKER="sudo"
fi

# Check 4: Dockerfile exists
echo -n "Checking Dockerfile... "
if [ -f "Dockerfile" ]; then
    echo -e "${GREEN}✓${NC}"
else
    echo -e "${RED}✗${NC}"
    echo "Dockerfile not found in current directory"
    exit 1
fi

# Check 5: docker-compose.yml exists
echo -n "Checking docker-compose.yml... "
if [ -f "docker-compose.yml" ]; then
    echo -e "${GREEN}✓${NC}"
    if command -v docker-compose &> /dev/null; then
        echo "  docker-compose version: $(docker-compose --version)"
    fi
else
    echo -e "${YELLOW}⚠${NC} (optional, not found)"
fi

# Check 6: Entrypoint script
echo -n "Checking entrypoint script... "
if [ -f "docker-entrypoint.sh" ] && [ -x "docker-entrypoint.sh" ]; then
    echo -e "${GREEN}✓${NC}"
elif [ -f "docker-entrypoint.sh" ]; then
    echo -e "${YELLOW}⚠${NC} (not executable)"
    chmod +x docker-entrypoint.sh
    echo "  Made executable"
else
    echo -e "${RED}✗${NC}"
    echo "docker-entrypoint.sh not found"
    exit 1
fi

# Check 7: Kernel modules (NBD and loop)
echo -n "Checking kernel modules availability... "
if lsmod | grep -q nbd && lsmod | grep -q loop; then
    echo -e "${GREEN}✓${NC} (both loaded)"
elif [ -f "/lib/modules/$(uname -r)/kernel/drivers/block/nbd.ko" ] || \
     [ -f "/lib/modules/$(uname -r)/kernel/drivers/block/nbd.ko.xz" ]; then
    echo -e "${YELLOW}⚠${NC} (available but not loaded)"
    echo "  NBD module available, will be loaded by container"
else
    echo -e "${YELLOW}⚠${NC} (may not be available)"
    echo "  NBD module may not be available on this kernel"
fi

# Check 8: Disk space
echo -n "Checking disk space... "
AVAILABLE_MB=$(df -m . | awk 'NR==2 {print $4}')
if [ "$AVAILABLE_MB" -gt 2000 ]; then
    echo -e "${GREEN}✓${NC} (${AVAILABLE_MB}MB available)"
elif [ "$AVAILABLE_MB" -gt 500 ]; then
    echo -e "${YELLOW}⚠${NC} (${AVAILABLE_MB}MB available)"
    echo "  Low disk space, build may succeed but be slow"
else
    echo -e "${RED}✗${NC} (${AVAILABLE_MB}MB available)"
    echo "  Insufficient disk space for Docker build (need ~2GB)"
fi

# Summary
echo
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo -e "${BLUE}Summary:${NC}"
echo

if [ "$CAN_RUN_DOCKER" = "direct" ]; then
    echo "✅ Ready to build Docker image!"
    echo
    echo "Next steps:"
    echo "  docker build -t guestkit:latest ."
    echo "  docker run --privileged -v ./vms:/vms:ro guestkit:latest --help"
elif [ "$CAN_RUN_DOCKER" = "sudo" ]; then
    echo "✅ Ready to build (with sudo)!"
    echo
    echo "Next steps:"
    echo "  sudo docker build -t guestkit:latest ."
    echo "  sudo docker run --privileged -v ./vms:/vms:ro guestkit:latest --help"
    echo
    echo "Alternatively, add yourself to docker group:"
    echo "  sudo usermod -aG docker \$USER"
    echo "  newgrp docker  # or logout/login"
fi

echo
echo "For detailed instructions, see DOCKER.md"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
