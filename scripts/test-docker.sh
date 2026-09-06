#!/bin/bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Test script for Docker deployment

set -e

echo "🐳 Testing guestkit Docker setup..."
echo

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    echo -e "${RED}❌ Docker not found. Please install Docker first.${NC}"
    exit 1
fi

echo -e "${GREEN}✓${NC} Docker installed"

# Check if running as root or in docker group
if [ "$EUID" -ne 0 ] && ! groups | grep -q docker; then
    echo -e "${YELLOW}⚠${NC}  Warning: Not running as root and not in docker group"
    echo "  You may need to use 'sudo docker' or add your user to docker group"
fi

# Build the image
echo
echo "📦 Building Docker image..."
if docker build -t guestkit:test .; then
    echo -e "${GREEN}✓${NC} Image built successfully"
else
    echo -e "${RED}❌ Build failed${NC}"
    exit 1
fi

# Test basic command
echo
echo "🧪 Testing basic command (--help)..."
if docker run --rm guestkit:test --help > /dev/null; then
    echo -e "${GREEN}✓${NC} Basic command works"
else
    echo -e "${RED}❌ Basic command failed${NC}"
    exit 1
fi

# Check image size
echo
echo "📊 Image information:"
docker images guestkit:test --format "  Size: {{.Size}}"

# Create test directory structure
echo
echo "📁 Setting up test directory structure..."
mkdir -p test-vms test-output
echo -e "${GREEN}✓${NC} Created test-vms/ and test-output/ directories"

# Test with docker-compose if available
if command -v docker-compose &> /dev/null; then
    echo
    echo "🐙 Testing docker-compose..."
    if docker-compose config > /dev/null 2>&1; then
        echo -e "${GREEN}✓${NC} docker-compose.yml is valid"
    else
        echo -e "${RED}❌ docker-compose.yml has errors${NC}"
    fi
fi

# Show sample usage
echo
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✅ Docker setup complete!"
echo
echo "Sample usage:"
echo
echo "  # Inspect a VM:"
echo "  docker run --privileged \\"
echo "    -v \$(pwd)/test-vms:/vms:ro \\"
echo "    guestkit:test inspect /vms/vm.qcow2"
echo
echo "  # Batch inspection:"
echo "  docker run --privileged \\"
echo "    -v \$(pwd)/test-vms:/vms:ro \\"
echo "    -v \$(pwd)/test-output:/output \\"
echo "    guestkit:test inspect-batch /vms/*.qcow2 --parallel 4"
echo
echo "  # With docker-compose:"
echo "  docker-compose run guestkit inspect /vms/vm.qcow2"
echo
echo "See DOCKER.md for more examples and production usage."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
