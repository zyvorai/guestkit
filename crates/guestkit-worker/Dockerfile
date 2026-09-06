# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Multi-stage build for guestkit worker
FROM rust:slim-bookworm AS builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libsystemd-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy the entire workspace
COPY crates/guestkit-job-spec /build/crates/guestkit-job-spec
COPY crates/guestkit-agent-protocol /build/crates/guestkit-agent-protocol
COPY crates/guestkit-worker /build/crates/guestkit-worker
COPY src /build/src
COPY benches /build/benches
COPY templates /build/templates
COPY Cargo.toml /build/

# Build the worker in release mode using manifest path
RUN cargo build --release --manifest-path crates/guestkit-worker/Cargo.toml --bin guestkit-worker

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies (loop/NBD disk access for assurance ops)
RUN apt-get update && apt-get install -y \
    qemu-utils \
    kmod \
    util-linux \
    parted \
    e2fsprogs \
    cpio \
    gzip \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create worker user
RUN useradd -r -u 1000 -m -s /bin/bash worker

# Create directories
RUN mkdir -p /var/lib/guestkit/jobs \
    /var/lib/guestkit/results \
    /var/lib/guestkit/work \
    /var/lib/guestkit/jobs/done \
    /var/lib/guestkit/jobs/failed \
    && chown -R worker:worker /var/lib/guestkit

# Copy binary
COPY --from=builder /build/crates/guestkit-worker/target/release/guestkit-worker /usr/local/bin/

# Run as root for privileged NBD/guestfs operations in Kubernetes
WORKDIR /var/lib/guestkit

VOLUME ["/var/lib/guestkit/jobs", "/var/lib/guestkit/results", "/var/lib/guestkit/work", "/var/lib/zyvor/images"]

HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD pgrep -f guestkit-worker || exit 1

ENTRYPOINT ["/usr/local/bin/guestkit-worker"]
CMD ["daemon", "--transport", "redis", "--results-dir", "/var/lib/guestkit/results", "--work-dir", "/var/lib/guestkit/work"]
