# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Reproducible CI build container for Zeus VM Tools (not a runtime service image).
FROM rust:1.85-bookworm AS builder

RUN apt-get update && apt-get install -y musl-tools && rm -rf /var/lib/apt/lists/*
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /build
COPY . .

ARG VERSION=0.0.0-dev
ENV VERSION="${VERSION}"

RUN bash packaging/vmtools/build-artifacts.sh

FROM scratch AS export
COPY --from=builder /build/dist/vmtools/ /vmtools/
