# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
FROM rust:slim-bookworm AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y pkg-config libssl-dev libsystemd-dev && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY crates/guestkit-job-spec /build/crates/guestkit-job-spec
COPY crates/guestkit-agent-protocol /build/crates/guestkit-agent-protocol
COPY crates/zyvor-guest-agent /build/crates/zyvor-guest-agent
COPY crates/zyvor-api /build/crates/zyvor-api
COPY src /build/src
COPY benches /build/benches
COPY templates /build/templates
RUN cargo build --release --manifest-path crates/zyvor-api/Cargo.toml \
    && cargo build --release --features agent --bin guestkit

FROM debian:bookworm-slim
# cpio + gzip: validate_initrd_boot_partition (src/guestfs/inspect.rs) shells
# out to `zcat <initrd> | cpio -t` to identify cirros-style cloud images
# (root filesystem inside the initrd, not on a directly-mountable
# partition). Without cpio here, that command silently fails and gets
# treated as "not a valid root" (by design — mount/extraction failures
# during inspection aren't supposed to be fatal) rather than as an error,
# so run_migrate_plan (called synchronously here, unlike guestkit-worker's
# async jobs) reports "No operating system found" for exactly the images
# guestkit-worker's identical Dockerfile package list already handles.
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    qemu-utils \
    kmod \
    util-linux \
    parted \
    e2fsprogs \
    cpio \
    gzip \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/crates/zyvor-api/target/release/zyvor-api /usr/local/bin/
COPY --from=builder /build/target/release/guestkit /usr/local/bin/guestkit
RUN mkdir -p /var/lib/zyvor/images
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/zyvor-api"]
