# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Deprecated for releases: use deploy/scripts/publish-images.sh and per-crate Dockerfiles.

# Multi-stage build for guestkit
# Stage 1: Builder
FROM rust:1.70-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src
COPY benches ./benches
COPY examples ./examples
COPY tests ./tests

# Build release binary
RUN cargo build --release --bin guestkit --bin guestctl

# Stage 2: Runtime
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    qemu-utils \
    kmod \
    util-linux \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /build/target/release/guestkit /usr/local/bin/guestkit
COPY --from=builder /build/target/release/guestctl /usr/local/bin/guestctl

# Create directory for VM images
RUN mkdir -p /vms /cache /config

# Set environment variables
ENV RUST_LOG=info \
    GUESTKIT_CACHE_DIR=/cache \
    GUESTKIT_CONFIG_DIR=/config

# Add entrypoint script
COPY docker-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

WORKDIR /vms

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["--help"]

# Stage 3: LVM Worker Container
# Pre-built image with all LVM/filesystem tools for podman-based clone isolation
FROM fedora:43 AS lvm-worker

LABEL description="guestkit LVM worker - privileged container for LVM clone operations"

RUN dnf install -y \
    lvm2 \
    e2fsprogs \
    xfsprogs \
    util-linux \
    cryptsetup \
    parted \
    kpartx \
    sudo \
    && dnf clean all

CMD ["sleep", "infinity"]
