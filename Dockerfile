# Multi-stage build for rust-loadtest
#
# Builder uses Alpine + musl to produce a fully static binary with no glibc
# dependency, so the runtime image works on any Linux distro regardless of
# the host glibc version.  reqwest already uses rustls (pure-Rust TLS) so
# OpenSSL is not required at build or runtime.
#
# Stage 1: Build static binary
FROM rust:alpine AS builder

WORKDIR /app

# musl-dev provides the musl libc headers for static linking
RUN apk add --no-cache musl-dev

# Copy manifests first for better layer caching
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src
COPY tests ./tests
COPY examples ./examples

# Add musl target and build a fully static release binary
RUN rustup target add x86_64-unknown-linux-musl
RUN cargo build --release --target x86_64-unknown-linux-musl

# Stage 2: Runtime (Debian for shell access, utilities, and CA certs)
FROM debian:bookworm-slim

WORKDIR /app

# ca-certificates provides the system CA store for rustls-native-roots.
# libssl3 is no longer needed — the binary uses rustls (pure-Rust TLS).
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the static binary — no glibc symbols referenced
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/rust_loadtest /usr/local/bin/rust-loadtest

# Copy example configs and data
COPY examples/configs /app/configs
COPY examples/data /app/data
COPY docs /app/docs

# Set working directory
WORKDIR /app

# Default command shows help
CMD ["rust-loadtest", "--help"]
