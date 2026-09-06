#!/bin/bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Deploy guestkit worker to k3d cluster

set -e

CLUSTER_NAME="${CLUSTER_NAME:-hyper2kvm-test}"
IMAGE_NAME="guestkit-worker:latest"

echo "=== Deploying Guestkit Worker to k3d ==="
echo

# Check if k3d cluster exists
if ! sudo k3d cluster list | grep -q "$CLUSTER_NAME"; then
    echo "ERROR: k3d cluster '$CLUSTER_NAME' not found"
    echo "Available clusters:"
    sudo k3d cluster list
    exit 1
fi

echo "✓ Using cluster: $CLUSTER_NAME"

# Build Docker image if not exists or force rebuild
if [ "$1" == "--build" ] || ! sudo docker images | grep -q "guestkit-worker"; then
    echo
    echo "=== Building Docker Image ==="
    cd "$(dirname "$0")/.."
    sudo docker build -f crates/guestkit-worker/Dockerfile -t "$IMAGE_NAME" .
    echo "✓ Image built: $IMAGE_NAME"
fi

# Import image to k3d
echo
echo "=== Importing Image to k3d ==="
sudo k3d image import "$IMAGE_NAME" -c "$CLUSTER_NAME"
echo "✓ Image imported to cluster"

# Label worker nodes
echo
echo "=== Labeling Worker Nodes ==="
for node in $(sudo kubectl get nodes -o name | grep agent); do
    sudo kubectl label --overwrite "$node" guestkit.io/worker-enabled=true
    echo "✓ Labeled $node"
done

# Deploy to cluster
echo
echo "=== Deploying Resources ==="
cd "$(dirname "$0")/.."
sudo kubectl apply -k k8s/
echo "✓ Resources deployed"

# Wait for pods to be ready
echo
echo "=== Waiting for Pods ==="
sudo kubectl wait --for=condition=Ready pods -l app=guestkit-worker -n guestkit-workers --timeout=120s || true

# Show status
echo
echo "=== Deployment Status ==="
sudo kubectl get all -n guestkit-workers

echo
echo "=== Pod Logs (last 10 lines) ==="
POD=$(sudo kubectl get pods -n guestkit-workers -l app=guestkit-worker -o name | head -1)
if [ -n "$POD" ]; then
    sudo kubectl logs -n guestkit-workers "$POD" --tail=10
fi

echo
echo "For full Zyvor stack (API, UI, Redis, Postgres), use:"
echo "  ./deploy/scripts/kind-kubevirt-quickstart.sh"
echo
echo "✅ Deployment complete!"
echo
echo "To check logs:"
echo "  sudo kubectl logs -n guestkit-workers -l app=guestkit-worker -f"
echo
echo "To submit a job:"
echo "  sudo kubectl cp job.json guestkit-workers/\${POD}:/var/lib/guestkit/jobs/"
